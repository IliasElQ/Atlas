use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::time::Duration;
use tracing::{debug, instrument, warn};

use crate::models::{JobsResponse, Repository, WorkflowRunsResponse};

// ── Constants ──────────────────────────────────────────────────────

const DEFAULT_BASE_URL: &str = "https://api.github.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: u32 = 3;

// ── GitHub API Client ──────────────────────────────────────────────

#[derive(Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
    pub owner: String,
    pub repo: String,
    base_url: String,
}

impl GitHubClient {
    /// Create a client for browsing (no repo selected yet).
    pub fn new_with_token(token: String) -> Self {
        Self::with_base_url(
            String::new(),
            String::new(),
            token,
            DEFAULT_BASE_URL.to_string(),
        )
    }

    /// Create a client with a custom API base URL and no repo (for GHE browsing).
    pub fn new_with_token_and_base(token: String, base_url: String) -> Self {
        Self::with_base_url(String::new(), String::new(), token, base_url)
    }

    pub fn new(owner: String, repo: String, token: String) -> Self {
        Self::with_base_url(owner, repo, token, DEFAULT_BASE_URL.to_string())
    }

    /// Create a client with a custom API base URL (for GitHub Enterprise).
    pub fn with_base_url(owner: String, repo: String, token: String, base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            token,
            owner,
            repo,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Switch to a different repository.
    pub fn set_repo(&mut self, owner: String, repo: String) {
        self.owner = owner;
        self.repo = repo;
    }

    // ── Core request engine with retry + rate-limit handling ───────

    async fn execute_with_retry(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                debug!(
                    attempt,
                    delay_ms = delay.as_millis() as u64,
                    "Retrying request"
                );
                tokio::time::sleep(delay).await;
            }

            let mut req = self
                .client
                .request(method.clone(), &url)
                .header(USER_AGENT, "atlas-prod-monitor")
                .header(ACCEPT, "application/vnd.github+json")
                .header(AUTHORIZATION, format!("Bearer {}", self.token));

            for (k, v) in query {
                req = req.query(&[(*k, v.as_str())]);
            }

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) if e.is_timeout() || e.is_connect() => {
                    warn!(attempt = attempt + 1, error = %e, "Request failed (transient)");
                    last_error = Some(e.into());
                    continue;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e).context("Request failed"));
                }
            };

            // Rate limit handling (429 or 403 with x-ratelimit-remaining: 0)
            let is_rate_limited = resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                || (resp.status() == reqwest::StatusCode::FORBIDDEN
                    && resp
                        .headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        == Some("0"));

            if is_rate_limited {
                let wait_secs = resp
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<i64>().ok())
                    .map(|reset| {
                        let now = chrono::Utc::now().timestamp();
                        (reset - now).clamp(1, 60) as u64
                    })
                    .unwrap_or(5);

                warn!(
                    wait_secs,
                    attempt = attempt + 1,
                    "Rate limited by GitHub API"
                );
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                last_error = Some(anyhow::anyhow!("Rate limited"));
                continue;
            }

            // Server errors are retryable
            if resp.status().is_server_error() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(%status, attempt = attempt + 1, "Server error (retryable)");
                last_error = Some(anyhow::anyhow!(
                    "GitHub API server error ({}): {}",
                    status,
                    body
                ));
                continue;
            }

            // Client errors (4xx except rate limit) are NOT retryable
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("GitHub API error ({}): {}", status, body);
            }

            return Ok(resp);
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("Request failed after {} retries", MAX_RETRIES)))
    }

    // ── API methods ────────────────────────────────────────────────

    /// Fetch user repositories (sorted by most recently pushed)
    #[instrument(skip(self))]
    pub async fn get_user_repos(&self, per_page: u8, page: u64) -> Result<Vec<Repository>> {
        let query = vec![
            ("per_page", per_page.to_string()),
            ("page", page.to_string()),
            ("sort", "pushed".to_string()),
            ("direction", "desc".to_string()),
            ("type", "all".to_string()),
        ];

        let resp = self
            .execute_with_retry(reqwest::Method::GET, "/user/repos", &query)
            .await
            .context("Failed to fetch repositories")?;

        resp.json::<Vec<Repository>>()
            .await
            .context("Failed to parse repositories response")
    }

    /// Fetch recent workflow runs for the repo
    #[instrument(skip(self), fields(owner = %self.owner, repo = %self.repo))]
    pub async fn get_workflow_runs(
        &self,
        per_page: u8,
        page: u64,
        branch: Option<&str>,
        status: Option<&str>,
    ) -> Result<WorkflowRunsResponse> {
        let path = format!("/repos/{}/{}/actions/runs", self.owner, self.repo);

        let mut query = vec![
            ("per_page", per_page.to_string()),
            ("page", page.to_string()),
        ];
        if let Some(branch) = branch {
            query.push(("branch", branch.to_string()));
        }
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }

        let resp = self
            .execute_with_retry(reqwest::Method::GET, &path, &query)
            .await
            .context("Failed to fetch workflow runs")?;

        resp.json::<WorkflowRunsResponse>()
            .await
            .context("Failed to parse workflow runs response")
    }

    /// Fetch jobs for a specific workflow run
    #[instrument(skip(self), fields(run_id))]
    pub async fn get_jobs(&self, run_id: u64) -> Result<JobsResponse> {
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/jobs",
            self.owner, self.repo, run_id
        );
        let query = vec![("per_page", "100".to_string())];

        let resp = self
            .execute_with_retry(reqwest::Method::GET, &path, &query)
            .await
            .context("Failed to fetch jobs")?;

        resp.json::<JobsResponse>()
            .await
            .context("Failed to parse jobs response")
    }

    /// Get logs for a specific job (returns raw text)
    #[instrument(skip(self), fields(job_id))]
    pub async fn get_job_logs(&self, job_id: u64) -> Result<String> {
        let path = format!(
            "/repos/{}/{}/actions/jobs/{}/logs",
            self.owner, self.repo, job_id
        );

        let resp = self
            .execute_with_retry(reqwest::Method::GET, &path, &[])
            .await
            .context("Failed to fetch job logs")?;

        resp.text().await.context("Failed to read log body")
    }

    /// Re-run a failed workflow run
    #[instrument(skip(self), fields(run_id))]
    pub async fn rerun_workflow(&self, run_id: u64) -> Result<()> {
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/rerun",
            self.owner, self.repo, run_id
        );

        self.execute_with_retry(reqwest::Method::POST, &path, &[])
            .await
            .context("Failed to re-run workflow")?;

        Ok(())
    }

    /// Cancel a workflow run
    #[instrument(skip(self), fields(run_id))]
    pub async fn cancel_workflow(&self, run_id: u64) -> Result<()> {
        let path = format!(
            "/repos/{}/{}/actions/runs/{}/cancel",
            self.owner, self.repo, run_id
        );

        self.execute_with_retry(reqwest::Method::POST, &path, &[])
            .await
            .context("Failed to cancel workflow")?;

        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client_default_base_url() {
        let client = GitHubClient::new("owner".into(), "repo".into(), "token".into());
        assert_eq!(client.base_url, DEFAULT_BASE_URL);
        assert_eq!(client.owner, "owner");
        assert_eq!(client.repo, "repo");
    }

    #[test]
    fn test_with_base_url_trims_trailing_slash() {
        let client = GitHubClient::with_base_url(
            "owner".into(),
            "repo".into(),
            "token".into(),
            "https://github.example.com/api/v3/".into(),
        );
        assert_eq!(client.base_url, "https://github.example.com/api/v3");
    }

    #[test]
    fn test_client_is_clone() {
        let client = GitHubClient::new("owner".into(), "repo".into(), "token".into());
        let cloned = client.clone();
        assert_eq!(cloned.owner, client.owner);
        assert_eq!(cloned.repo, client.repo);
        assert_eq!(cloned.base_url, client.base_url);
    }
}

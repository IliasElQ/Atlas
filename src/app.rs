use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::github::GitHubClient;
use crate::models::{Job, JobsResponse, Repository, WorkflowRun, WorkflowRunsResponse};

// ── App views ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    RepoList,
    RunsList,
    RunDetail,
    Logs,
}

// ── Background task results ────────────────────────────────────────

pub enum BackgroundResult {
    ReposFetched(Result<Vec<Repository>>),
    RunsFetched(Result<WorkflowRunsResponse>),
    JobsFetched {
        run_number: u64,
        result: Result<JobsResponse>,
    },
    LogsFetched {
        job_name: String,
        result: Result<String>,
    },
    RerunComplete {
        run_number: u64,
        result: Result<()>,
    },
    CancelComplete {
        run_number: u64,
        result: Result<()>,
    },
}

// ── App state ──────────────────────────────────────────────────────

pub struct App {
    pub client: GitHubClient,
    pub view: View,
    pub should_quit: bool,

    // Background task channel
    bg_tx: mpsc::UnboundedSender<BackgroundResult>,

    // Repository list
    pub repos: Vec<Repository>,
    pub repos_selected: usize,
    pub repo_filter: String,
    pub searching: bool,

    // Runs list
    pub runs: Vec<WorkflowRun>,
    pub runs_selected: usize,
    pub runs_total: u64,
    pub page: u64,
    pub per_page: u8,

    // Run detail (jobs + steps)
    pub current_run: Option<WorkflowRun>,
    pub jobs: Vec<Job>,
    pub jobs_selected: usize,

    // Logs (usize avoids u16 overflow on large logs)
    pub log_content: Vec<String>,
    pub log_scroll: usize,

    // Status bar messages
    pub status_message: String,
    pub loading: bool,
}

impl App {
    /// Create app in multi-repo browser mode (starts at RepoList)
    pub fn new_browser(
        client: GitHubClient,
        bg_tx: mpsc::UnboundedSender<BackgroundResult>,
    ) -> Self {
        Self {
            client,
            view: View::RepoList,
            should_quit: false,
            bg_tx,

            repos: Vec::new(),
            repos_selected: 0,
            repo_filter: String::new(),
            searching: false,

            runs: Vec::new(),
            runs_selected: 0,
            runs_total: 0,
            page: 1,
            per_page: 20,

            current_run: None,
            jobs: Vec::new(),
            jobs_selected: 0,

            log_content: Vec::new(),
            log_scroll: 0,

            status_message: String::from("Loading repositories..."),
            loading: true,
        }
    }

    /// Create app in single-repo mode (starts at RunsList)
    pub fn new(client: GitHubClient, bg_tx: mpsc::UnboundedSender<BackgroundResult>) -> Self {
        Self {
            view: View::RunsList,
            status_message: String::from("Loading..."),
            ..Self::new_browser(client, bg_tx)
        }
    }

    // ── Filtered repos helper ──────────────────────────────────────

    /// Returns repos filtered by the current search string
    pub fn filtered_repos(&self) -> Vec<&Repository> {
        if self.repo_filter.is_empty() {
            self.repos.iter().collect()
        } else {
            let q = self.repo_filter.to_lowercase();
            self.repos
                .iter()
                .filter(|r| {
                    r.full_name.to_lowercase().contains(&q)
                        || r.description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&q)
                        || r.language
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&q)
                })
                .collect()
        }
    }

    // ── Search mode ────────────────────────────────────────────────

    pub fn start_search(&mut self) {
        if self.view == View::RepoList {
            self.searching = true;
        }
    }

    pub fn stop_search(&mut self) {
        self.searching = false;
    }

    pub fn search_push(&mut self, c: char) {
        self.repo_filter.push(c);
        self.repos_selected = 0;
        self.update_repo_status();
    }

    pub fn search_backspace(&mut self) {
        self.repo_filter.pop();
        self.repos_selected = 0;
        self.update_repo_status();
    }

    pub fn search_clear(&mut self) {
        if self.repo_filter.is_empty() {
            self.searching = false;
        } else {
            self.repo_filter.clear();
            self.repos_selected = 0;
            self.update_repo_status();
        }
    }

    fn update_repo_status(&mut self) {
        let filtered = self.filtered_repos();
        let total = self.repos.len();
        let shown = filtered.len();
        if self.repo_filter.is_empty() {
            self.status_message = format!("{} repositories", total);
        } else {
            self.status_message = format!(
                "{} / {} repos matching \"{}\"",
                shown, total, self.repo_filter
            );
        }
    }

    // ── Background task spawning (non-blocking) ────────────────────

    pub fn spawn_fetch_repos(&mut self) {
        self.loading = true;
        self.status_message = "Fetching repositories...".to_string();

        let client = self.client.clone();
        let tx = self.bg_tx.clone();

        tokio::spawn(async move {
            debug!("Fetching user repositories");
            let result = client.get_user_repos(100, 1).await;
            let _ = tx.send(BackgroundResult::ReposFetched(result));
        });
    }

    pub fn spawn_fetch_runs(&mut self) {
        self.loading = true;
        self.status_message = "Fetching workflow runs...".to_string();

        let client = self.client.clone();
        let per_page = self.per_page;
        let page = self.page;
        let tx = self.bg_tx.clone();

        tokio::spawn(async move {
            debug!(page, per_page, "Fetching workflow runs");
            let result = client.get_workflow_runs(per_page, page, None, None).await;
            let _ = tx.send(BackgroundResult::RunsFetched(result));
        });
    }

    pub fn spawn_fetch_jobs(&mut self) {
        if let Some(run) = &self.current_run {
            self.loading = true;
            self.status_message = format!("Fetching jobs for run #{}...", run.run_number);

            let client = self.client.clone();
            let run_id = run.id;
            let run_number = run.run_number;
            let tx = self.bg_tx.clone();

            tokio::spawn(async move {
                debug!(run_id, run_number, "Fetching jobs");
                let result = client.get_jobs(run_id).await;
                let _ = tx.send(BackgroundResult::JobsFetched { run_number, result });
            });
        }
    }

    pub fn spawn_fetch_logs(&mut self) {
        if let Some(job) = self.jobs.get(self.jobs_selected) {
            self.loading = true;
            self.status_message = format!("Fetching logs for {}...", job.name);

            let client = self.client.clone();
            let job_id = job.id;
            let job_name = job.name.clone();
            let tx = self.bg_tx.clone();

            tokio::spawn(async move {
                debug!(job_id, %job_name, "Fetching logs");
                let result = client.get_job_logs(job_id).await;
                let _ = tx.send(BackgroundResult::LogsFetched { job_name, result });
            });
        }
    }

    pub fn spawn_rerun(&mut self) {
        if let Some(run) = self.get_selected_run() {
            self.status_message = format!("Re-running workflow #{}...", run.run_number);

            let client = self.client.clone();
            let run_id = run.id;
            let run_number = run.run_number;
            let tx = self.bg_tx.clone();

            tokio::spawn(async move {
                debug!(run_id, run_number, "Re-running workflow");
                let result = client.rerun_workflow(run_id).await;
                let _ = tx.send(BackgroundResult::RerunComplete { run_number, result });
            });
        }
    }

    pub fn spawn_cancel(&mut self) {
        if let Some(run) = self.get_selected_run() {
            self.status_message = format!("Cancelling workflow #{}...", run.run_number);

            let client = self.client.clone();
            let run_id = run.id;
            let run_number = run.run_number;
            let tx = self.bg_tx.clone();

            tokio::spawn(async move {
                debug!(run_id, run_number, "Cancelling workflow");
                let result = client.cancel_workflow(run_id).await;
                let _ = tx.send(BackgroundResult::CancelComplete { run_number, result });
            });
        }
    }

    fn get_selected_run(&self) -> Option<WorkflowRun> {
        match self.view {
            View::RunsList => self.runs.get(self.runs_selected).cloned(),
            View::RunDetail | View::Logs => self.current_run.clone(),
            View::RepoList => None,
        }
    }

    // ── Handle background results ──────────────────────────────────

    pub fn handle_background(&mut self, result: BackgroundResult) {
        match result {
            BackgroundResult::ReposFetched(result) => match result {
                Ok(repos) => {
                    let count = repos.len();
                    self.repos = repos;
                    self.loading = false;
                    self.repos_selected = 0;
                    self.status_message =
                        format!("{} repositories · sorted by last push · / to search", count,);
                    debug!(count, "Repositories fetched");
                }
                Err(e) => {
                    self.loading = false;
                    self.status_message = format!("Error: {}", e);
                    error!(error = %e, "Failed to fetch repositories");
                }
            },

            BackgroundResult::RunsFetched(result) => match result {
                Ok(response) => {
                    self.runs = response.workflow_runs;
                    self.runs_total = response.total_count;
                    self.loading = false;

                    let total_pages = self.runs_total.div_ceil(self.per_page as u64);
                    self.status_message = format!(
                        "{} runs total · Page {}/{} · {} {}",
                        self.runs_total,
                        self.page,
                        total_pages,
                        self.client.owner,
                        self.client.repo,
                    );
                    debug!(total = self.runs_total, page = self.page, "Runs fetched");
                }
                Err(e) => {
                    self.loading = false;
                    self.status_message = format!("Error: {}", e);
                    error!(error = %e, "Failed to fetch runs");
                }
            },

            BackgroundResult::JobsFetched { run_number, result } => match result {
                Ok(response) => {
                    self.jobs = response.jobs;
                    self.jobs_selected = 0;
                    self.loading = false;

                    let run_name = self
                        .current_run
                        .as_ref()
                        .and_then(|r| r.display_title.as_deref().or(r.name.as_deref()))
                        .unwrap_or("Unknown");
                    self.status_message = format!(
                        "Run #{} · {} · {} jobs",
                        run_number,
                        run_name,
                        self.jobs.len()
                    );
                    debug!(run_number, jobs = self.jobs.len(), "Jobs fetched");
                }
                Err(e) => {
                    self.loading = false;
                    self.status_message = format!("Error: {}", e);
                    error!(error = %e, run_number, "Failed to fetch jobs");
                }
            },

            BackgroundResult::LogsFetched { job_name, result } => match result {
                Ok(logs) => {
                    self.log_content = logs.lines().map(|l| l.to_string()).collect();
                    self.log_scroll = 0;
                    self.loading = false;
                    self.status_message =
                        format!("Logs: {} · {} lines", job_name, self.log_content.len());
                    debug!(%job_name, lines = self.log_content.len(), "Logs fetched");
                }
                Err(e) => {
                    self.log_content = vec![format!("Error fetching logs: {}", e)];
                    self.loading = false;
                    self.status_message = format!("Failed to load logs for {}", job_name);
                    error!(error = %e, %job_name, "Failed to fetch logs");
                }
            },

            BackgroundResult::RerunComplete { run_number, result } => match result {
                Ok(()) => {
                    self.status_message = format!("✓ Re-run triggered for #{}", run_number);
                    debug!(run_number, "Re-run triggered");
                }
                Err(e) => {
                    self.status_message = format!("Error: {}", e);
                    error!(error = %e, run_number, "Failed to re-run");
                }
            },

            BackgroundResult::CancelComplete { run_number, result } => match result {
                Ok(()) => {
                    self.status_message = format!("✓ Cancelled #{}", run_number);
                    debug!(run_number, "Workflow cancelled");
                }
                Err(e) => {
                    self.status_message = format!("Error: {}", e);
                    error!(error = %e, run_number, "Failed to cancel");
                }
            },
        }
    }

    // ── Navigation ─────────────────────────────────────────────────

    pub fn move_up(&mut self) {
        match self.view {
            View::RepoList => {
                if self.repos_selected > 0 {
                    self.repos_selected -= 1;
                }
            }
            View::RunsList => {
                if self.runs_selected > 0 {
                    self.runs_selected -= 1;
                }
            }
            View::RunDetail => {
                if self.jobs_selected > 0 {
                    self.jobs_selected -= 1;
                }
            }
            View::Logs => {
                self.log_scroll = self.log_scroll.saturating_sub(3);
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.view {
            View::RepoList => {
                let count = self.filtered_repos().len();
                if count > 0 && self.repos_selected < count - 1 {
                    self.repos_selected += 1;
                }
            }
            View::RunsList => {
                if !self.runs.is_empty() && self.runs_selected < self.runs.len() - 1 {
                    self.runs_selected += 1;
                }
            }
            View::RunDetail => {
                if !self.jobs.is_empty() && self.jobs_selected < self.jobs.len() - 1 {
                    self.jobs_selected += 1;
                }
            }
            View::Logs => {
                let max_scroll = self.log_content.len().saturating_sub(10);
                self.log_scroll = (self.log_scroll + 3).min(max_scroll);
            }
        }
    }

    pub fn enter(&mut self) {
        match self.view {
            View::RepoList => {
                let filtered = self.filtered_repos();
                if let Some(repo) = filtered.get(self.repos_selected).cloned() {
                    let owner = repo.owner.login.clone();
                    let repo_name = repo.name.clone();
                    self.client.set_repo(owner, repo_name);
                    self.view = View::RunsList;
                    self.runs.clear();
                    self.runs_selected = 0;
                    self.runs_total = 0;
                    self.page = 1;
                    self.repo_filter.clear();
                    self.searching = false;
                    self.spawn_fetch_runs();
                }
            }
            View::RunsList => {
                if let Some(run) = self.runs.get(self.runs_selected).cloned() {
                    self.current_run = Some(run);
                    self.view = View::RunDetail;
                    self.spawn_fetch_jobs();
                }
            }
            View::RunDetail => {
                self.view = View::Logs;
                self.spawn_fetch_logs();
            }
            View::Logs => {}
        }
    }

    pub fn back(&mut self) {
        match self.view {
            View::RepoList => {
                self.should_quit = true;
            }
            View::RunsList => {
                // Go back to repo list (or quit if in single-repo mode)
                if self.repos.is_empty() {
                    self.should_quit = true;
                } else {
                    self.view = View::RepoList;
                    self.runs.clear();
                    self.runs_selected = 0;
                    self.update_repo_status();
                }
            }
            View::RunDetail => {
                self.view = View::RunsList;
                self.current_run = None;
                self.jobs.clear();
            }
            View::Logs => {
                self.view = View::RunDetail;
                self.log_content.clear();
                self.log_scroll = 0;
            }
        }
    }

    pub fn next_page(&mut self) {
        if self.view == View::RunsList {
            let total_pages = self.runs_total.div_ceil(self.per_page as u64);
            if self.page < total_pages {
                self.page += 1;
                self.runs_selected = 0;
                self.spawn_fetch_runs();
            }
        }
    }

    pub fn prev_page(&mut self) {
        if self.view == View::RunsList && self.page > 1 {
            self.page -= 1;
            self.runs_selected = 0;
            self.spawn_fetch_runs();
        }
    }

    pub fn refresh(&mut self) {
        match self.view {
            View::RepoList => self.spawn_fetch_repos(),
            View::RunsList => self.spawn_fetch_runs(),
            View::RunDetail => self.spawn_fetch_jobs(),
            View::Logs => self.spawn_fetch_logs(),
        }
    }

    pub fn open_in_browser(&self) {
        let url = match self.view {
            View::RepoList => {
                let filtered = self.filtered_repos();
                filtered
                    .get(self.repos_selected)
                    .map(|r| r.html_url.clone())
            }
            View::RunsList => self
                .runs
                .get(self.runs_selected)
                .map(|r| r.html_url.clone()),
            View::RunDetail | View::Logs => {
                if let Some(job) = self.jobs.get(self.jobs_selected) {
                    job.html_url.clone()
                } else {
                    self.current_run.as_ref().map(|r| r.html_url.clone())
                }
            }
        };

        if let Some(url) = url {
            let _ = open::that(&url);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GitHubClient;

    fn test_app() -> (App, mpsc::UnboundedReceiver<BackgroundResult>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = GitHubClient::new("owner".into(), "repo".into(), "token".into());
        (App::new(client, tx), rx)
    }

    fn test_browser_app() -> (App, mpsc::UnboundedReceiver<BackgroundResult>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = GitHubClient::new_with_token("token".into());
        (App::new_browser(client, tx), rx)
    }

    #[test]
    fn test_initial_state() {
        let (app, _rx) = test_app();
        assert_eq!(app.view, View::RunsList);
        assert!(!app.should_quit);
        assert_eq!(app.page, 1);
        assert_eq!(app.runs_selected, 0);
    }

    #[test]
    fn test_browser_initial_state() {
        let (app, _rx) = test_browser_app();
        assert_eq!(app.view, View::RepoList);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_move_up_at_zero_stays() {
        let (mut app, _rx) = test_app();
        app.runs_selected = 0;
        app.move_up();
        assert_eq!(app.runs_selected, 0);
    }

    #[test]
    fn test_move_down_empty_list() {
        let (mut app, _rx) = test_app();
        app.move_down();
        assert_eq!(app.runs_selected, 0);
    }

    #[test]
    fn test_back_from_runs_single_repo_quits() {
        let (mut app, _rx) = test_app();
        app.view = View::RunsList;
        app.back();
        assert!(app.should_quit);
    }

    #[test]
    fn test_back_from_detail_goes_to_list() {
        let (mut app, _rx) = test_app();
        app.view = View::RunDetail;
        app.back();
        assert_eq!(app.view, View::RunsList);
        assert!(app.current_run.is_none());
    }

    #[test]
    fn test_back_from_logs_goes_to_detail() {
        let (mut app, _rx) = test_app();
        app.view = View::Logs;
        app.log_content = vec!["line1".into()];
        app.log_scroll = 5;
        app.back();
        assert_eq!(app.view, View::RunDetail);
        assert!(app.log_content.is_empty());
        assert_eq!(app.log_scroll, 0);
    }

    #[test]
    fn test_log_scroll_large_values() {
        let (mut app, _rx) = test_app();
        app.view = View::Logs;
        app.log_content = (0..100_000).map(|i| format!("line {}", i)).collect();
        app.log_scroll = 99_980;
        app.move_down();
        assert!(app.log_scroll <= app.log_content.len());
    }

    #[test]
    fn test_log_scroll_saturating_sub() {
        let (mut app, _rx) = test_app();
        app.view = View::Logs;
        app.log_content = vec!["a".into(); 20];
        app.log_scroll = 1;
        app.move_up();
        assert_eq!(app.log_scroll, 0);
    }

    #[test]
    fn test_search_mode() {
        let (mut app, _rx) = test_browser_app();
        assert!(!app.searching);
        app.start_search();
        assert!(app.searching);
        app.search_push('t');
        app.search_push('e');
        assert_eq!(app.repo_filter, "te");
        app.search_backspace();
        assert_eq!(app.repo_filter, "t");
        app.search_clear();
        assert_eq!(app.repo_filter, "");
        assert!(app.searching);
        app.search_clear();
        assert!(!app.searching);
    }

    #[test]
    fn test_back_from_repo_list_quits() {
        let (mut app, _rx) = test_browser_app();
        app.view = View::RepoList;
        app.back();
        assert!(app.should_quit);
    }
}

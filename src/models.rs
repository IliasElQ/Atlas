use chrono::{DateTime, Utc};
use serde::Deserialize;

// ── Repository types ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    #[allow(dead_code)]
    pub id: u64,
    pub full_name: String,
    pub name: String,
    pub owner: RepoOwner,
    pub description: Option<String>,
    pub html_url: String,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub updated_at: DateTime<Utc>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub private: bool,
    #[allow(dead_code)]
    pub fork: bool,
    #[allow(dead_code)]
    pub archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoOwner {
    pub login: String,
}

impl Repository {
    /// Human-readable "last active" string
    pub fn last_active_display(&self) -> String {
        let ts = self.pushed_at.unwrap_or(self.updated_at);
        let secs = Utc::now().signed_duration_since(ts).num_seconds();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }

    #[allow(dead_code)]
    pub fn visibility_icon(&self) -> &str {
        if self.private {
            "🔒"
        } else {
            "🌍"
        }
    }
}

// ── GitHub API response types ──────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowRunsResponse {
    pub total_count: u64,
    pub workflow_runs: Vec<WorkflowRun>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: Option<String>,
    pub display_title: Option<String>,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub run_number: u64,
    pub event: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub actor: Option<Actor>,
    #[allow(dead_code)]
    pub run_attempt: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Actor {
    pub login: String,
    #[allow(dead_code)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobsResponse {
    #[allow(dead_code)]
    pub total_count: u64,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Job {
    pub id: u64,
    #[allow(dead_code)]
    pub run_id: u64,
    pub name: String,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Option<Vec<Step>>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    #[allow(dead_code)]
    pub number: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── Display helpers ────────────────────────────────────────────────

impl WorkflowRun {
    pub fn status_display(&self) -> &str {
        match self.conclusion.as_deref() {
            Some("success") => "✓ Success",
            Some("failure") => "✗ Failure",
            Some("cancelled") => "⊘ Cancelled",
            Some("skipped") => "⊘ Skipped",
            Some("timed_out") => "⏱ Timed Out",
            Some(other) => other,
            None => match self.status.as_deref() {
                Some("queued") => "◯ Queued",
                Some("in_progress") => "● In Progress",
                Some("waiting") => "◎ Waiting",
                Some(other) => other,
                None => "? Unknown",
            },
        }
    }

    pub fn duration_display(&self) -> String {
        if let Some(started) = self.run_started_at {
            let end = if self.status.as_deref() == Some("completed") {
                self.updated_at
            } else {
                Utc::now()
            };
            let dur = end.signed_duration_since(started);
            let secs = dur.num_seconds();
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m {}s", secs / 60, secs % 60)
            } else {
                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
            }
        } else {
            "—".to_string()
        }
    }

    pub fn short_sha(&self) -> &str {
        if self.head_sha.len() >= 7 {
            &self.head_sha[..7]
        } else {
            &self.head_sha
        }
    }

    pub fn age_display(&self) -> String {
        let dur = Utc::now().signed_duration_since(self.created_at);
        let secs = dur.num_seconds();
        if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
}

impl Job {
    pub fn status_display(&self) -> &str {
        match self.conclusion.as_deref() {
            Some("success") => "✓ Success",
            Some("failure") => "✗ Failure",
            Some("cancelled") => "⊘ Cancelled",
            Some("skipped") => "⊘ Skipped",
            _ => match self.status.as_deref() {
                Some("queued") => "◯ Queued",
                Some("in_progress") => "● Running",
                Some("waiting") => "◎ Waiting",
                _ => "? Unknown",
            },
        }
    }

    pub fn duration_display(&self) -> String {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => {
                let secs = end.signed_duration_since(start).num_seconds();
                if secs < 60 {
                    format!("{}s", secs)
                } else if secs < 3600 {
                    format!("{}m {}s", secs / 60, secs % 60)
                } else {
                    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                }
            }
            (Some(start), None) => {
                let secs = Utc::now().signed_duration_since(start).num_seconds();
                format!("{}s (running)", secs)
            }
            _ => "—".to_string(),
        }
    }
}

impl Step {
    pub fn status_icon(&self) -> &str {
        match self.conclusion.as_deref() {
            Some("success") => "✓",
            Some("failure") => "✗",
            Some("cancelled") => "⊘",
            Some("skipped") => "⊘",
            _ => match self.status.as_str() {
                "in_progress" => "●",
                "queued" => "◯",
                _ => "?",
            },
        }
    }

    pub fn duration_display(&self) -> String {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => {
                let secs = end.signed_duration_since(start).num_seconds();
                if secs < 60 {
                    format!("{}s", secs)
                } else {
                    format!("{}m {}s", secs / 60, secs % 60)
                }
            }
            _ => "—".to_string(),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_run(status: Option<&str>, conclusion: Option<&str>) -> WorkflowRun {
        WorkflowRun {
            id: 1,
            name: Some("CI".to_string()),
            display_title: Some("Fix bug".to_string()),
            head_branch: Some("main".to_string()),
            head_sha: "abc1234567890".to_string(),
            status: status.map(String::from),
            conclusion: conclusion.map(String::from),
            run_number: 42,
            event: "push".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            run_started_at: Some(Utc::now()),
            html_url: "https://github.com/test/repo/actions/runs/1".to_string(),
            actor: Some(Actor {
                login: "testuser".to_string(),
                avatar_url: None,
            }),
            run_attempt: Some(1),
        }
    }

    #[test]
    fn test_status_display_success() {
        let run = make_run(Some("completed"), Some("success"));
        assert_eq!(run.status_display(), "✓ Success");
    }

    #[test]
    fn test_status_display_failure() {
        let run = make_run(Some("completed"), Some("failure"));
        assert_eq!(run.status_display(), "✗ Failure");
    }

    #[test]
    fn test_status_display_cancelled() {
        let run = make_run(Some("completed"), Some("cancelled"));
        assert_eq!(run.status_display(), "⊘ Cancelled");
    }

    #[test]
    fn test_status_display_in_progress() {
        let run = make_run(Some("in_progress"), None);
        assert_eq!(run.status_display(), "● In Progress");
    }

    #[test]
    fn test_status_display_queued() {
        let run = make_run(Some("queued"), None);
        assert_eq!(run.status_display(), "◯ Queued");
    }

    #[test]
    fn test_status_display_unknown() {
        let run = make_run(None, None);
        assert_eq!(run.status_display(), "? Unknown");
    }

    #[test]
    fn test_short_sha() {
        let run = make_run(None, None);
        assert_eq!(run.short_sha(), "abc1234");
    }

    #[test]
    fn test_short_sha_short_input() {
        let mut run = make_run(None, None);
        run.head_sha = "abc".to_string();
        assert_eq!(run.short_sha(), "abc");
    }

    #[test]
    fn test_duration_display_seconds() {
        let mut run = make_run(Some("completed"), Some("success"));
        let started = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ended = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 45).unwrap();
        run.run_started_at = Some(started);
        run.updated_at = ended;
        run.status = Some("completed".to_string());
        assert_eq!(run.duration_display(), "45s");
    }

    #[test]
    fn test_duration_display_minutes() {
        let mut run = make_run(Some("completed"), Some("success"));
        let started = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ended = Utc.with_ymd_and_hms(2025, 1, 1, 0, 2, 30).unwrap();
        run.run_started_at = Some(started);
        run.updated_at = ended;
        run.status = Some("completed".to_string());
        assert_eq!(run.duration_display(), "2m 30s");
    }

    #[test]
    fn test_duration_display_hours() {
        let mut run = make_run(Some("completed"), Some("success"));
        let started = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ended = Utc.with_ymd_and_hms(2025, 1, 1, 1, 30, 0).unwrap();
        run.run_started_at = Some(started);
        run.updated_at = ended;
        run.status = Some("completed".to_string());
        assert_eq!(run.duration_display(), "1h 30m");
    }

    #[test]
    fn test_duration_display_no_start() {
        let mut run = make_run(Some("completed"), Some("success"));
        run.run_started_at = None;
        assert_eq!(run.duration_display(), "—");
    }

    #[test]
    fn test_job_status_display() {
        let job = Job {
            id: 1,
            run_id: 1,
            name: "build".to_string(),
            status: Some("completed".to_string()),
            conclusion: Some("success".to_string()),
            started_at: None,
            completed_at: None,
            steps: None,
            html_url: None,
        };
        assert_eq!(job.status_display(), "✓ Success");
    }

    #[test]
    fn test_job_duration_display() {
        let started = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ended = Utc.with_ymd_and_hms(2025, 1, 1, 0, 1, 15).unwrap();
        let job = Job {
            id: 1,
            run_id: 1,
            name: "build".to_string(),
            status: Some("completed".to_string()),
            conclusion: Some("success".to_string()),
            started_at: Some(started),
            completed_at: Some(ended),
            steps: None,
            html_url: None,
        };
        assert_eq!(job.duration_display(), "1m 15s");
    }

    #[test]
    fn test_step_status_icon() {
        let step = Step {
            name: "Checkout".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
            number: 1,
            started_at: None,
            completed_at: None,
        };
        assert_eq!(step.status_icon(), "✓");

        let step_fail = Step {
            conclusion: Some("failure".to_string()),
            ..step.clone()
        };
        assert_eq!(step_fail.status_icon(), "✗");

        let step_skip = Step {
            conclusion: Some("skipped".to_string()),
            ..step.clone()
        };
        assert_eq!(step_skip.status_icon(), "⊘");
    }

    #[test]
    fn test_step_duration_display() {
        let started = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ended = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 30).unwrap();
        let step = Step {
            name: "Build".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
            number: 1,
            started_at: Some(started),
            completed_at: Some(ended),
        };
        assert_eq!(step.duration_display(), "30s");
    }

    #[test]
    fn test_age_display() {
        // Just verify it doesn't panic and returns a string with "ago"
        let run = make_run(None, None);
        let age = run.age_display();
        assert!(age.contains("ago"));
    }
}

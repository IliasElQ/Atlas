mod app;
mod auth;
mod event;
mod github;
mod models;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{Event, EventStream, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

use app::View;
use app::{App, BackgroundResult};
use event::{map_key_to_action, Action};
use github::GitHubClient;

// ── CLI Arguments ──────────────────────────────────────────────────

/// Atlas — Production Monitor for GitHub Actions (GitLab coming soon)
#[derive(Parser, Debug)]
#[command(
    name = "atlas",
    version,
    about = "Atlas | Production Monitor for GitHub & GitLab -- by Ilias El Qadiri"
)]
struct Cli {
    /// GitHub repository (owner/repo). Defaults to current git repo.
    #[arg(short, long, global = true)]
    repo: Option<String>,

    /// GitHub personal access token. Overrides stored credentials.
    #[arg(short, long, global = true)]
    token: Option<String>,

    /// GitHub API base URL (for GitHub Enterprise).
    /// Defaults to https://api.github.com
    #[arg(long, global = true, env = "GITHUB_API_URL")]
    api_url: Option<String>,

    /// Enable debug logging to ~/.atlas/atlas.log
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage GitHub authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

#[derive(Subcommand, Debug)]
enum AuthAction {
    /// Log in to GitHub (opens browser or paste token)
    Login {
        /// GitHub OAuth App Client ID (for device flow)
        #[arg(long)]
        client_id: Option<String>,
    },
    /// Log out and remove stored credentials
    Logout,
    /// Show current authentication status
    Status,
}

// ── Tracing ────────────────────────────────────────────────────────

fn init_tracing(verbose: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if !verbose {
        return None;
    }

    let log_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".atlas");

    if std::fs::create_dir_all(&log_dir).is_err() {
        eprintln!("Warning: Could not create log directory {:?}", log_dir);
        return None;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "atlas.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "atlas=debug".into()),
        )
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .init();

    Some(guard)
}

// ── Terminal safety ────────────────────────────────────────────────

/// Install a panic hook that restores the terminal before printing the panic.
/// Without this, a panic leaves the terminal in raw mode (unusable).
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restore
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));
}

/// Restore the terminal to its normal state (always called, even on error).
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

// ── Splash screen ──────────────────────────────────────────────────

/// Print a colorful startup splash before entering the TUI
/// Get terminal width, with a sensible fallback
fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}

/// Pad a string so it appears centered in the terminal
fn center(text: &str, width: usize) -> String {
    let stripped_len = strip_ansi_len(text);
    if stripped_len >= width {
        return text.to_string();
    }
    let pad = (width - stripped_len) / 2;
    format!("{}{}", " ".repeat(pad), text)
}

/// Count visible (non-ANSI) character width of a string
fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c.is_ascii_alphabetic() {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        // Unicode block chars are generally 1 column wide in terminals
        len += 1;
    }
    len
}

/// Print a colorful startup splash before entering the TUI
fn print_splash(owner: &str, repo: &str) {
    use std::io::Write;

    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const ITALIC: &str = "\x1b[3m";

    // Gradient: purple → blue → cyan → mint
    const C1: &str = "\x1b[38;2;190;80;250m";
    const C2: &str = "\x1b[38;2;170;88;252m";
    const C3: &str = "\x1b[38;2;150;96;255m";
    const C4: &str = "\x1b[38;2;130;115;255m";
    const C5: &str = "\x1b[38;2;110;140;255m";
    const C6: &str = "\x1b[38;2;88;166;255m";
    const C7: &str = "\x1b[38;2;60;190;230m";
    const C8: &str = "\x1b[38;2;50;210;200m";
    const C9: &str = "\x1b[38;2;72;220;170m";
    const GOLD: &str = "\x1b[38;2;255;215;0m";
    const WHITE: &str = "\x1b[38;2;230;237;243m";
    const MAG: &str = "\x1b[38;2;188;140;255m";
    const SILVER: &str = "\x1b[38;2;160;170;180m";

    let w = term_width();

    // Big ANSI Shadow ATLAS (9 lines tall)
    let art: &[(&str, &str)] = &[
        (C1, "  ██████╗   ██████████╗  ██╗           ██████╗    ████████╗"),
        (C2, " ██╔══██╗   ╚═══██╔═══╝  ██║          ██╔══██╗   ██╔══════╝"),
        (C3, "██║    ██║      ██║      ██║         ██║    ██║  ██║       "),
        (C4, "██║    ██║      ██║      ██║         ██║    ██║  ╚███████╗ "),
        (C5, "█████████║      ██║      ██║         █████████║   ╚═════██║"),
        (C6, "██╔════██║      ██║      ██║         ██╔════██║         ██║"),
        (C7, "██║    ██║      ██║      ██║         ██║    ██║         ██║"),
        (C8, "██║    ██║      ██║      █████████╗  ██║    ██║  █████████║"),
        (C9, "╚═╝    ╚═╝      ╚═╝      ╚════════╝  ╚═╝    ╚═╝  ╚════════╝"),
    ];

    // Stylish "of prod" subtitle
    let subtitle = format!("{SILVER}{DIM}{ITALIC}-- of prod --{RESET}");

    println!();
    for (color, line) in art {
        let centered = center(line, w);
        let padded = format!("{color}{centered}{RESET}");
        for ch in padded.chars() {
            print!("{ch}");
            let _ = io::stdout().flush();
        }
        println!();
        std::thread::sleep(Duration::from_millis(30));
    }
    println!("{}", center(&subtitle, w));
    std::thread::sleep(Duration::from_millis(50));

    // Dynamic divider
    let div_inner = w.saturating_sub(4).max(20);
    let divider = format!(
        "{GOLD}◆{RESET}{DIM}{}{RESET}{GOLD}◆{RESET}",
        "━".repeat(div_inner)
    );
    println!();
    println!("{}", center(&divider, w));
    std::thread::sleep(Duration::from_millis(40));

    let title = format!(
        "{C3}{BOLD}Atlas{RESET} {DIM}v{}{RESET}  {DIM}│{RESET}  {WHITE}GitHub Actions Monitor{RESET}",
        env!("CARGO_PKG_VERSION")
    );
    let repo_line = format!(
        "{DIM}Monitoring{RESET} {MAG}{BOLD}{}/{}{RESET}  {DIM}│{RESET}  {DIM}GitLab coming soon{RESET}",
        owner,
        repo
    );
    println!("{}", center(&title, w));
    println!("{}", center(&repo_line, w));

    println!("{}", center(&divider, w));
    println!();

    std::thread::sleep(Duration::from_millis(200));
}

/// Print a startup splash for browser mode (no specific repo)
fn print_splash_browser() {
    use std::io::Write;

    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const DIM: &str = "\x1b[2m";
    const ITALIC: &str = "\x1b[3m";

    const C1: &str = "\x1b[38;2;190;80;250m";
    const C2: &str = "\x1b[38;2;170;88;252m";
    const C3: &str = "\x1b[38;2;150;96;255m";
    const C4: &str = "\x1b[38;2;130;115;255m";
    const C5: &str = "\x1b[38;2;110;140;255m";
    const C6: &str = "\x1b[38;2;88;166;255m";
    const C7: &str = "\x1b[38;2;60;190;230m";
    const C8: &str = "\x1b[38;2;50;210;200m";
    const C9: &str = "\x1b[38;2;72;220;170m";
    const GOLD: &str = "\x1b[38;2;255;215;0m";
    const WHITE: &str = "\x1b[38;2;230;237;243m";
    const SILVER: &str = "\x1b[38;2;160;170;180m";

    let w = term_width();

    let art: &[(&str, &str)] = &[
        (C1, "  ██████╗   ██████████╗  ██╗           ██████╗    ████████╗"),
        (C2, " ██╔══██╗   ╚═══██╔═══╝  ██║          ██╔══██╗   ██╔══════╝"),
        (C3, "██║    ██║      ██║      ██║         ██║    ██║  ██║       "),
        (C4, "██║    ██║      ██║      ██║         ██║    ██║  ╚███████╗ "),
        (C5, "█████████║      ██║      ██║         █████████║   ╚═════██║"),
        (C6, "██╔════██║      ██║      ██║         ██╔════██║         ██║"),
        (C7, "██║    ██║      ██║      ██║         ██║    ██║         ██║"),
        (C8, "██║    ██║      ██║      █████████╗  ██║    ██║  █████████║"),
        (C9, "╚═╝    ╚═╝      ╚═╝      ╚════════╝  ╚═╝    ╚═╝  ╚════════╝"),
    ];

    let subtitle = format!("{SILVER}{DIM}{ITALIC}-- of prod --{RESET}");

    println!();
    for (color, line) in art {
        let centered = center(line, w);
        println!("{color}{centered}{RESET}");
        std::thread::sleep(Duration::from_millis(25));
    }
    println!("{}", center(&subtitle, w));
    std::thread::sleep(Duration::from_millis(50));

    let div_inner = w.saturating_sub(4).max(20);
    let divider = format!(
        "{GOLD}◆{RESET}{DIM}{}{RESET}{GOLD}◆{RESET}",
        "━".repeat(div_inner)
    );
    println!();
    println!("{}", center(&divider, w));

    let title = format!(
        "{C3}{BOLD}Atlas{RESET} {DIM}v{}{RESET}  {DIM}│{RESET}  {WHITE}GitHub Actions Monitor{RESET}",
        env!("CARGO_PKG_VERSION")
    );
    let browse_line = format!(
        "{DIM}Browsing all repositories{RESET}  {DIM}│{RESET}  {DIM}GitLab coming soon{RESET}"
    );
    println!("{}", center(&title, w));
    println!("{}", center(&browse_line, w));

    println!("{}", center(&divider, w));
    println!();

    let loading = format!("{C5}Loading your repos...{RESET}");
    print!("{}", center(&loading, w));
    let _ = io::stdout().flush();
    std::thread::sleep(Duration::from_millis(200));
    println!();
}

// ── Main ───────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing (file-based, only when --verbose is set)
    let _guard = init_tracing(cli.verbose);

    // Install panic hook BEFORE terminal setup
    install_panic_hook();

    info!("Atlas starting");

    // Handle subcommands
    match cli.command {
        Some(Commands::Auth { action }) => {
            return handle_auth(action).await;
        }
        None => {
            // Default: launch the TUI
        }
    }

    // Resolve token (CLI flag -> env var -> keychain -> interactive login)
    let token = auth::resolve_token(cli.token).await?;

    // Determine mode: single-repo or multi-repo browser
    let single_repo = if let Some(repo_arg) = &cli.repo {
        Some(parse_repo(repo_arg)?)
    } else {
        // Try to detect from git, but don't fail — fall back to browser mode
        detect_repo_from_git().ok()
    };

    // Create background task channel
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();

    let mut app = if let Some((owner, repo)) = single_repo {
        info!(%owner, %repo, "Single-repo mode");
        print_splash(&owner, &repo);

        let client = if let Some(api_url) = cli.api_url {
            GitHubClient::with_base_url(owner, repo, token, api_url)
        } else {
            GitHubClient::new(owner, repo, token)
        };

        let mut app = App::new(client, bg_tx);
        app.spawn_fetch_runs();
        app
    } else {
        info!("Multi-repo browser mode");
        print_splash_browser();

        let client = if let Some(api_url) = cli.api_url {
            GitHubClient::new_with_token_and_base(token, api_url)
        } else {
            GitHubClient::new_with_token(token)
        };

        let mut app = App::new_browser(client, bg_tx);
        app.spawn_fetch_repos();
        app
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the async event loop
    let result = run_app(&mut terminal, &mut app, bg_rx).await;

    // Restore terminal (always, even on error)
    restore_terminal(&mut terminal);

    info!("Atlas exiting");

    result
}

async fn handle_auth(action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login { client_id } => auth::login(client_id.as_deref()).await,
        AuthAction::Logout => auth::logout(),
        AuthAction::Status => auth::status().await,
    }
}

// ── Async event loop ───────────────────────────────────────────────

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut bg_rx: mpsc::UnboundedReceiver<BackgroundResult>,
) -> Result<()> {
    let mut reader = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // Draw
        terminal.draw(|f| ui::draw(f, app))?;

        // Wait for next event (fully non-blocking via tokio::select!)
        tokio::select! {
            // Keyboard / terminal events (async via crossterm EventStream)
            maybe_event = reader.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        // Search mode: route key presses to the filter
                        if app.searching && app.view == View::RepoList {
                            use crossterm::event::KeyCode;
                            match key.code {
                                KeyCode::Esc => app.search_clear(),
                                KeyCode::Backspace => app.search_backspace(),
                                KeyCode::Enter => { app.stop_search(); app.enter(); }
                                KeyCode::Up => app.move_up(),
                                KeyCode::Down => app.move_down(),
                                KeyCode::Char(c) => app.search_push(c),
                                _ => {}
                            }
                        } else {
                            let action = map_key_to_action(key);
                            match action {
                                Action::Quit => app.should_quit = true,
                                Action::MoveUp => app.move_up(),
                                Action::MoveDown => app.move_down(),
                                Action::Enter => app.enter(),
                                Action::Back => app.back(),
                                Action::Refresh => app.refresh(),
                                Action::NextPage => app.next_page(),
                                Action::PrevPage => app.prev_page(),
                                Action::ToggleLogs => app.spawn_fetch_logs(),
                                Action::Rerun => app.spawn_rerun(),
                                Action::Cancel => app.spawn_cancel(),
                                Action::OpenInBrowser => app.open_in_browser(),
                                Action::Search => app.start_search(),
                                Action::None => {}
                            }
                        }
                    }
                    Some(Ok(_)) => {} // Ignore non-key events (resize, mouse, etc.)
                    Some(Err(e)) => {
                        app.status_message = format!("Input error: {}", e);
                    }
                    None => break, // Stream ended
                }
            }

            // Background task results (non-blocking receive)
            Some(result) = bg_rx.recv() => {
                app.handle_background(result);
            }

            // Tick (for future auto-refresh or animations)
            _ = tick.tick() => {}
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────

fn parse_repo(input: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        anyhow::bail!("Invalid repo format: '{}'. Expected 'owner/repo'", input);
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn detect_repo_from_git() -> Result<(String, String)> {
    // Try 'origin' first, then fall back to any remote that points to GitHub
    let remotes_to_try = ["origin", "upstream", "github"];

    for remote in &remotes_to_try {
        if let Ok(result) = try_remote(remote) {
            return Ok(result);
        }
    }

    // None of the well-known names worked — enumerate all remotes
    let list_output = std::process::Command::new("git")
        .args(["remote"])
        .output()
        .context("Failed to run 'git remote'. Is this a git repository?")?;

    if list_output.status.success() {
        let all = String::from_utf8_lossy(&list_output.stdout);
        for name in all.lines() {
            let name = name.trim();
            if !name.is_empty() && !remotes_to_try.contains(&name) {
                if let Ok(result) = try_remote(name) {
                    return Ok(result);
                }
            }
        }
    }

    anyhow::bail!(
        "No GitHub remote found.\n\
         Either:\n  \
           • Add a remote:  git remote add origin https://github.com/OWNER/REPO.git\n  \
           • Or pass:       atlas --repo owner/repo"
    )
}

fn try_remote(name: &str) -> Result<(String, String)> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", name])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("remote '{}' not found", name);
    }

    let url = String::from_utf8(output.stdout)?.trim().to_string();
    parse_github_url(&url)
}

fn parse_github_url(url: &str) -> Result<(String, String)> {
    // Handle SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let clean = rest.trim_end_matches(".git");
        return parse_repo(clean);
    }

    // Handle HTTPS: https://github.com/owner/repo.git
    if url.contains("github.com") {
        let parts: Vec<&str> = url.split("github.com/").collect();
        if parts.len() == 2 {
            let clean = parts[1].trim_end_matches(".git");
            return parse_repo(clean);
        }
    }

    anyhow::bail!("Could not parse GitHub URL: {}", url)
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_valid() {
        let (owner, repo) = parse_repo("octocat/hello-world").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "hello-world");
    }

    #[test]
    fn test_parse_repo_invalid_no_slash() {
        assert!(parse_repo("invalid").is_err());
    }

    #[test]
    fn test_parse_repo_invalid_empty_parts() {
        assert!(parse_repo("/repo").is_err());
        assert!(parse_repo("owner/").is_err());
        assert!(parse_repo("/").is_err());
    }

    #[test]
    fn test_parse_repo_too_many_slashes() {
        assert!(parse_repo("a/b/c").is_err());
    }

    #[test]
    fn test_parse_github_url_ssh() {
        let (owner, repo) = parse_github_url("git@github.com:octocat/hello-world.git").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "hello-world");
    }

    #[test]
    fn test_parse_github_url_ssh_no_suffix() {
        let (owner, repo) = parse_github_url("git@github.com:octocat/hello-world").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "hello-world");
    }

    #[test]
    fn test_parse_github_url_https() {
        let (owner, repo) = parse_github_url("https://github.com/octocat/hello-world.git").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "hello-world");
    }

    #[test]
    fn test_parse_github_url_https_no_suffix() {
        let (owner, repo) = parse_github_url("https://github.com/octocat/hello-world").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "hello-world");
    }

    #[test]
    fn test_parse_github_url_invalid() {
        assert!(parse_github_url("https://gitlab.com/foo/bar").is_err());
        assert!(parse_github_url("not-a-url").is_err());
    }
}

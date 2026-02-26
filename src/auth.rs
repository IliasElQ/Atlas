use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

// ── Constants ──────────────────────────────────────────────────────

const KEYRING_SERVICE: &str = "atlas-prod-monitor";
const KEYRING_USER: &str = "github-token";

// GitHub OAuth Device Flow endpoints
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

// ── ANSI Color helpers ─────────────────────────────────────────────

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const UNDERLINE: &str = "\x1b[4m";

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";

const BRIGHT_BLUE: &str = "\x1b[94m";
const BRIGHT_CYAN: &str = "\x1b[96m";
const BRIGHT_MAGENTA: &str = "\x1b[95m";

// ── Terminal centering helpers ──────────────────────────────────────

fn auth_term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}

fn auth_center(text: &str, width: usize) -> String {
    let stripped_len = auth_strip_ansi_len(text);
    if stripped_len >= width {
        return text.to_string();
    }
    let pad = (width - stripped_len) / 2;
    format!("{}{}", " ".repeat(pad), text)
}

fn auth_strip_ansi_len(s: &str) -> usize {
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
        len += 1;
    }
    len
}

// ── Animated ASCII Art Banner ──────────────────────────────────────

fn print_animated_banner() {
    // Gradient colors: magenta -> blue -> cyan for the large text
    const C1: &str = "\x1b[38;2;190;80;250m"; // purple
    const C2: &str = "\x1b[38;2;170;88;252m";
    const C3: &str = "\x1b[38;2;150;96;255m";
    const C4: &str = "\x1b[38;2;130;115;255m";
    const C5: &str = "\x1b[38;2;110;140;255m"; // bright blue
    const C6: &str = "\x1b[38;2;88;166;255m";
    const C7: &str = "\x1b[38;2;60;190;230m"; // sky blue
    const C8: &str = "\x1b[38;2;50;210;200m"; // teal
    const C9: &str = "\x1b[38;2;72;220;170m"; // mint green
    const SPARK: &str = "\x1b[38;2;255;215;0m"; // gold

    let lines: &[(&str, &str)] = &[
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
    const SILVER: &str = "\x1b[38;2;160;170;180m";
    const ITALIC_S: &str = "\x1b[3m";
    let subtitle = format!("{SILVER}{DIM}{ITALIC_S}-- of prod --{RESET}");

    let w = auth_term_width();

    println!();
    println!();

    // Animate each line with a sweep effect
    for (color, line) in lines {
        let centered = auth_center(line, w);
        let padded = format!("{color}{centered}{RESET}");
        for ch in padded.chars() {
            print!("{ch}");
            io::stdout().flush().unwrap_or(());
        }
        println!();
        thread::sleep(Duration::from_millis(35));
    }
    println!("{}", auth_center(&subtitle, w));

    thread::sleep(Duration::from_millis(80));

    // Dynamic divider
    let div_inner = w.saturating_sub(4).max(20);
    let divider = format!(
        "{SPARK}◆{RESET}{DIM}{}{RESET}{SPARK}◆{RESET}",
        "━".repeat(div_inner)
    );
    println!("{}", auth_center(&divider, w));
    thread::sleep(Duration::from_millis(50));

    // Title line with gradient
    let title = format!(
        "{C3}{BOLD}Atlas{RESET} {DIM}v{}{RESET}  {DIM}│{RESET}  {WHITE}GitHub Actions Monitor{RESET}",
        env!("CARGO_PKG_VERSION")
    );
    let centered_title = auth_center(&title, w);
    for ch in centered_title.chars() {
        print!("{ch}");
        io::stdout().flush().unwrap_or(());
    }
    println!();
    thread::sleep(Duration::from_millis(40));

    // Credit + GitLab teaser
    let credit = format!(
        "{DIM}Engineered by{RESET} {BRIGHT_MAGENTA}{BOLD}Ilias El Qadiri{RESET}  {DIM}│ GitLab coming soon{RESET}"
    );
    println!("{}", auth_center(&credit, w));
    thread::sleep(Duration::from_millis(40));

    println!("{}", auth_center(&divider, w));
    println!();
}

fn print_small_header() {
    const SPARK: &str = "\x1b[38;2;255;215;0m";
    const C3: &str = "\x1b[38;2;88;166;255m";
    let w = auth_term_width();
    let title = format!("{SPARK}◆{RESET} {C3}{BOLD}Atlas{RESET} {DIM}v{}{RESET} {DIM}│{RESET} {WHITE}GitHub Actions Monitor{RESET}", env!("CARGO_PKG_VERSION"));
    let credit = format!("{DIM}{ITALIC}Engineered by{RESET} {BRIGHT_MAGENTA}Ilias El Qadiri{RESET}  {DIM}│ GitLab coming soon{RESET}");
    println!();
    println!("{}", auth_center(&title, w));
    println!("{}", auth_center(&credit, w));
    println!();
}

// ── Keychain operations ────────────────────────────────────────────

/// Store a token securely in the system keychain
pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(token)
        .context("Failed to store token in keychain")?;

    // Verify the round-trip immediately
    match entry.get_password() {
        Ok(readback) if readback == token => {
            debug!("Keychain round-trip verified OK");
        }
        Ok(_) => {
            warn!("Keychain round-trip produced a different value");
        }
        Err(e) => {
            warn!("Keychain round-trip read-back failed: {}", e);
        }
    }

    Ok(())
}

/// Retrieve the stored token from the system keychain
pub fn get_stored_token() -> Option<String> {
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => match entry.get_password() {
            Ok(token) if !token.is_empty() => {
                debug!("Retrieved token from keychain");
                Some(token)
            }
            Ok(_) => {
                debug!("Keychain entry exists but is empty");
                None
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No token in keychain (NoEntry)");
                None
            }
            Err(e) => {
                warn!("Keychain read failed: {}", e);
                None
            }
        },
        Err(e) => {
            warn!("Could not create keyring entry: {}", e);
            None
        }
    }
}

/// Delete the stored token from the system keychain
pub fn delete_token() -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("Failed to access keyring entry")?;
    match entry.delete_credential() {
        Ok(()) => {
            debug!("Token deleted from keychain");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Failed to delete from keychain: {}", e)),
    }
}

// ── Token resolution ───────────────────────────────────────────────

/// Resolve a GitHub token from multiple sources (in priority order):
/// 1. CLI --token flag
/// 2. GITHUB_TOKEN env var
/// 3. GH_TOKEN env var
/// 4. System keychain
/// 5. If nothing found -> animated banner + interactive login
pub async fn resolve_token(cli_token: Option<String>) -> Result<String> {
    if let Some(token) = cli_token {
        return Ok(token);
    }

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    if let Some(token) = get_stored_token() {
        return Ok(token);
    }

    // No token anywhere -> show animated banner and prompt login
    print_animated_banner();

    println!("  {YELLOW}{BOLD}Not authenticated.{RESET}");
    println!("  {DIM}Let's get you set up. This only takes a moment.{RESET}");
    println!();

    let token = login_prompt().await?;
    Ok(token)
}

// ── Device Flow structs ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

// ── Auth Commands ──────────────────────────────────────────────────

/// Login prompt (no extra banner — used inline from resolve_token)
fn login_prompt() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + 'static>>
{
    Box::pin(async move {
        print_auth_menu();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => login_via_browser().await,
            "2" => login_via_paste().await,
            _ => {
                println!("  {DIM}Invalid choice. Please enter 1 or 2.{RESET}");
                println!();
                login_prompt().await
            }
        }
    })
}

/// Login entry point for `atlas auth login` subcommand
pub async fn login(client_id: Option<&str>) -> Result<()> {
    print_animated_banner();

    if let Some(cid) = client_id {
        // Direct device flow with a real client ID
        login_device_flow(cid).await?;
        return Ok(());
    }

    print_auth_menu();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;

    match choice.trim() {
        "1" => {
            login_via_browser().await?;
        }
        "2" => {
            login_via_paste().await?;
        }
        _ => {
            println!("  {DIM}Invalid choice. Please enter 1 or 2.{RESET}");
            println!();
        }
    }
    Ok(())
}

fn print_auth_menu() {
    println!("  {DIM}+--------------------------------------------------+{RESET}");
    println!("  {DIM}|{RESET}                                                  {DIM}|{RESET}");
    println!("  {DIM}|{RESET}  {BOLD}How would you like to authenticate?{RESET}              {DIM}|{RESET}");
    println!("  {DIM}|{RESET}                                                  {DIM}|{RESET}");
    println!("  {DIM}|{RESET}  {BRIGHT_CYAN}{BOLD}[1]{RESET}  Login with browser                         {DIM}|{RESET}");
    println!("  {DIM}|{RESET}       {DIM}Opens GitHub to create a new token,{RESET}        {DIM}|{RESET}");
    println!("  {DIM}|{RESET}       {DIM}then paste it back here.{RESET}                   {DIM}|{RESET}");
    println!("  {DIM}|{RESET}                                                  {DIM}|{RESET}");
    println!("  {DIM}|{RESET}  {BRIGHT_MAGENTA}{BOLD}[2]{RESET}  Paste an existing token                    {DIM}|{RESET}");
    println!("  {DIM}|{RESET}       {DIM}Already have a token? Paste it directly.{RESET}    {DIM}|{RESET}");
    println!("  {DIM}|{RESET}                                                  {DIM}|{RESET}");
    println!("  {DIM}+--------------------------------------------------+{RESET}");
    println!();
    print!("  {CYAN}>{RESET} Your choice {DIM}(1/2):{RESET} ");
    io::stdout().flush().unwrap_or(());
}

/// Option 1: Open browser to GitHub token creation page, then paste
async fn login_via_browser() -> Result<String> {
    println!();
    println!("  {DIM}----------------------------------------------------{RESET}");
    println!("  {BOLD}Browser Authentication{RESET}");
    println!("  {DIM}----------------------------------------------------{RESET}");
    println!();
    println!("  Opening GitHub in your browser...");
    println!("  {DIM}A new token page will open with the right scopes.{RESET}");
    println!();

    let _ = open::that("https://github.com/settings/tokens/new?scopes=repo,workflow&description=atlas-prod-monitor");

    println!("  {DIM}Steps:{RESET}");
    println!("  {DIM}  1. Set an expiration (or no expiration){RESET}");
    println!("  {DIM}  2. Click \"Generate token\" at the bottom{RESET}");
    println!("  {DIM}  3. Copy the token (starts with ghp_){RESET}");
    println!("  {DIM}  4. Paste it below{RESET}");
    println!();

    print!("  {CYAN}>{RESET} Paste your token: ");
    io::stdout().flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        anyhow::bail!("No token provided");
    }

    validate_and_store_token(&token).await
}

/// Option 2: Directly paste an existing token
async fn login_via_paste() -> Result<String> {
    println!();
    println!("  {DIM}----------------------------------------------------{RESET}");
    println!("  {BOLD}Token Authentication{RESET}");
    println!("  {DIM}----------------------------------------------------{RESET}");
    println!();
    println!("  {DIM}Paste a GitHub Personal Access Token with{RESET}");
    println!("  {DIM}scopes:{RESET} {BOLD}repo{RESET} {DIM}and{RESET} {BOLD}workflow{RESET}");
    println!();
    println!("  {DIM}Create one at:{RESET}");
    println!("  {UNDERLINE}{BRIGHT_BLUE}https://github.com/settings/tokens/new{RESET}");
    println!();

    print!("  {CYAN}>{RESET} Token: ");
    io::stdout().flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        anyhow::bail!("No token provided");
    }

    validate_and_store_token(&token).await
}

/// Validate a token against GitHub API and store in keychain
async fn validate_and_store_token(token: &str) -> Result<String> {
    println!();
    print!("  {DIM}Verifying with GitHub...{RESET}");
    io::stdout().flush()?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/user")
        .header("User-Agent", "atlas-prod-monitor")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !resp.status().is_success() {
        println!(" {RED}FAILED{RESET}");
        println!();
        anyhow::bail!(
            "Invalid token (HTTP {}). Make sure it has 'repo' scope.",
            resp.status()
        );
    }

    #[derive(Deserialize)]
    struct User {
        login: String,
    }
    let user: User = resp.json().await?;
    println!(" {GREEN}OK{RESET}");

    // Best-effort keychain storage (token is returned directly regardless)
    match store_token(token) {
        Ok(()) => {
            println!();
            println!("  {DIM}===================================================={RESET}");
            println!("  {GREEN}{BOLD}  Authentication successful!{RESET}");
            println!("  {DIM}----------------------------------------------------{RESET}");
            println!(
                "  {GREEN}[+]{RESET} Logged in as: {BOLD}{}{RESET}",
                user.login
            );
            println!("  {GREEN}[+]{RESET} Token stored securely in system keychain");
            println!("  {DIM}===================================================={RESET}");
        }
        Err(e) => {
            println!();
            println!("  {DIM}===================================================={RESET}");
            println!("  {GREEN}{BOLD}  Authentication successful!{RESET}");
            println!("  {DIM}----------------------------------------------------{RESET}");
            println!(
                "  {GREEN}[+]{RESET} Logged in as: {BOLD}{}{RESET}",
                user.login
            );
            println!(
                "  {YELLOW}[!]{RESET} Could not save to keychain: {DIM}{}{RESET}",
                e
            );
            println!("  {DIM}    Token will be used for this session only.{RESET}");
            println!("  {DIM}    Set GITHUB_TOKEN env var for persistence.{RESET}");
            println!("  {DIM}===================================================={RESET}");
        }
    }
    println!();
    println!("  You can now run {BOLD}atlas{RESET} to launch the dashboard.");
    println!();

    Ok(token.to_string())
}

/// Login via GitHub Device Flow (when a real client ID is provided)
async fn login_device_flow(cid: &str) -> Result<()> {
    let client = reqwest::Client::new();

    println!();
    println!("  {DIM}Requesting device code...{RESET}");
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", cid), ("scope", "repo,workflow")])
        .send()
        .await
        .context("Failed to request device code")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Device code request failed: {}", body);
    }

    let device: DeviceCodeResponse = resp.json().await?;

    println!();
    println!("  {DIM}+-------------------------------------------+{RESET}");
    println!("  {DIM}|{RESET}                                           {DIM}|{RESET}");
    println!("  {DIM}|{RESET}   Enter this code on GitHub:               {DIM}|{RESET}");
    println!("  {DIM}|{RESET}                                           {DIM}|{RESET}");
    println!(
        "  {DIM}|{RESET}          {YELLOW}{BOLD}  {}  {RESET}                      {DIM}|{RESET}",
        device.user_code
    );
    println!("  {DIM}|{RESET}                                           {DIM}|{RESET}");
    println!(
        "  {DIM}|{RESET}   {UNDERLINE}{BRIGHT_BLUE}{}{RESET}   {DIM}|{RESET}",
        device.verification_uri
    );
    println!("  {DIM}|{RESET}                                           {DIM}|{RESET}");
    println!("  {DIM}+-------------------------------------------+{RESET}");
    println!();

    // Copy code to clipboard (best-effort, macOS)
    if let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(device.user_code.as_bytes());
        }
        let _ = child.wait();
        println!("  {DIM}(Code copied to clipboard){RESET}");
    }

    let _ = open::that(&device.verification_uri);
    println!("  {DIM}Opening browser...{RESET}");
    println!();
    println!("  Waiting for authorization... {DIM}(Ctrl+C to abort){RESET}");
    println!();

    let interval = Duration::from_secs(device.interval.max(5));
    let deadline = std::time::Instant::now() + Duration::from_secs(device.expires_in);

    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("Authorization timed out. Please try again.");
        }

        tokio::time::sleep(interval).await;

        let resp = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", cid),
                ("device_code", device.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let token_resp: AccessTokenResponse = resp.json().await?;

        if let Some(access_token) = token_resp.access_token {
            validate_and_store_token(&access_token).await?;
            return Ok(());
        }

        match token_resp.error.as_deref() {
            Some("authorization_pending") => {
                print!("  {DIM}.{RESET}");
                io::stdout().flush()?;
            }
            Some("slow_down") => {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Some("expired_token") => {
                anyhow::bail!("Device code expired. Please try again.");
            }
            Some("access_denied") => {
                anyhow::bail!("Authorization denied by user.");
            }
            Some(err) => {
                let desc = token_resp.error_description.unwrap_or_default();
                anyhow::bail!("Authorization error: {} -- {}", err, desc);
            }
            None => {}
        }
    }
}

/// Show current auth status
pub async fn status() -> Result<()> {
    print_small_header();

    println!("  {DIM}--- Authentication Status ---{RESET}");
    println!();

    match get_stored_token() {
        Some(token) => {
            let masked = mask_token(&token);
            println!("  {GREEN}[+]{RESET} Keychain: {DIM}{}{RESET}", masked);

            print!("  {DIM}    Verifying...{RESET}");
            io::stdout().flush()?;

            let client = reqwest::Client::new();
            let resp = client
                .get("https://api.github.com/user")
                .header("User-Agent", "atlas-prod-monitor")
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    #[derive(Deserialize)]
                    struct User {
                        login: String,
                        name: Option<String>,
                    }
                    let user: User = r.json().await?;
                    println!(" {GREEN}OK{RESET}");
                    println!(
                        "  {GREEN}[+]{RESET} Logged in as: {BOLD}{}{RESET}{}",
                        user.login,
                        user.name
                            .map(|n| format!(" {DIM}({}){RESET}", n))
                            .unwrap_or_default()
                    );
                }
                Ok(r) => {
                    println!(" {RED}FAILED{RESET}");
                    println!(
                        "  {RED}[!]{RESET} Token is invalid or expired {DIM}(HTTP {}){RESET}",
                        r.status()
                    );
                    println!("  {DIM}    Run: atlas auth login{RESET}");
                }
                Err(e) => {
                    println!(" {RED}ERROR{RESET}");
                    println!(
                        "  {RED}[!]{RESET} Could not reach GitHub: {DIM}{}{RESET}",
                        e
                    );
                }
            }
        }
        None => {
            println!("  {YELLOW}[-]{RESET} Keychain: {DIM}no token stored{RESET}");
        }
    }

    println!();
    if let Ok(val) = std::env::var("GITHUB_TOKEN") {
        if !val.is_empty() {
            println!(
                "  {GREEN}[+]{RESET} GITHUB_TOKEN: {DIM}{}{RESET}",
                mask_token(&val)
            );
        }
    } else {
        println!("  {DIM}[ ]{RESET} GITHUB_TOKEN: {DIM}not set{RESET}");
    }

    if let Ok(val) = std::env::var("GH_TOKEN") {
        if !val.is_empty() {
            println!(
                "  {GREEN}[+]{RESET} GH_TOKEN:     {DIM}{}{RESET}",
                mask_token(&val)
            );
        }
    } else {
        println!("  {DIM}[ ]{RESET} GH_TOKEN:     {DIM}not set{RESET}");
    }

    println!();
    println!("  {DIM}Priority: --token > GITHUB_TOKEN > GH_TOKEN > keychain{RESET}");
    println!();

    Ok(())
}

/// Logout -- remove stored credentials
pub fn logout() -> Result<()> {
    print_small_header();

    match get_stored_token() {
        Some(_) => {
            delete_token()?;
            println!("  {GREEN}[+]{RESET} Token removed from system keychain");
            println!();
            println!("  {DIM}Note: This does not revoke the token on GitHub.{RESET}");
            println!("  {DIM}To revoke: https://github.com/settings/tokens{RESET}");
            println!();
        }
        None => {
            println!("  {DIM}[ ] No token found in keychain (already logged out){RESET}");
            println!();
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

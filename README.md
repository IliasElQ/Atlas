<p align="center">
  <br>
  <code>█████╗ ████████╗██╗      █████╗ ███████╗</code><br>
  <code>██╔══██╗╚══██╔══╝██║     ██╔══██╗██╔════╝</code><br>
  <code>███████║   ██║   ██║     ███████║███████╗</code><br>
  <code>██╔══██║   ██║   ██║     ██╔══██║╚════██║</code><br>
  <code>██║  ██║   ██║   ███████╗██║  ██║███████║</code><br>
  <code>╚═╝  ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝╚══════╝</code><br>
  <br>
  <strong>A terminal UI for monitoring GitHub Actions.</strong><br>
  <sub>Built with Rust + Ratatui</sub>
</p>

<p align="center">
  <a href="https://github.com/iliaselqadiri/atlas-prod-monitor/actions/workflows/ci.yml"><img src="https://github.com/iliaselqadiri/atlas-prod-monitor/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/Built%20with-Rust-orange?style=flat-square" alt="Rust">
  <img src="https://img.shields.io/badge/TUI-Ratatui-blue?style=flat-square" alt="Ratatui">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="MIT License"></a>
</p>

---

## Features

- **Dashboard** — Color-coded workflow runs with status, branch, duration, actor
- **Run details** — Drill into jobs and steps with timing info
- **Job logs** — Browse logs with syntax highlighting for errors/warnings
- **Actions** — Re-run, cancel workflows, open in browser — all from the terminal
- **Auth** — OAuth device flow, keychain storage, or plain env vars
- **Auto-detect** — Picks up repo from your current git directory
- **Vim keybindings** — `j`/`k`/`h`/`l`, arrows, and more
- **GitHub Enterprise** — Custom API URL support

## Install

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
./target/release/atlas
```

## Quick Start

```bash
# Authenticate via OAuth device flow (stores token in keychain)
atlas auth login

# Or set a token manually
export GITHUB_TOKEN="ghp_..."

# Run from inside a git repo (auto-detects owner/repo)
atlas

# Or specify a repo
atlas --repo owner/repo
```

## Authentication

Atlas resolves tokens in this order:

1. `--token` flag
2. `GITHUB_TOKEN` / `GH_TOKEN` env var
3. System keychain (stored via `atlas auth login`)

| Command | Description |
|---|---|
| `atlas auth login` | Authenticate via OAuth device flow |
| `atlas auth logout` | Remove stored credentials |
| `atlas auth status` | Show current auth status |

To create a token manually: [github.com/settings/tokens](https://github.com/settings/tokens) — needs **repo** scope.

### GitHub Enterprise

```bash
export GITHUB_API_URL="https://github.example.com/api/v3"
atlas --repo owner/repo
```

## Keybindings

### Runs List

| Key | Action |
|---|---|
| `↑` `k` | Move up |
| `↓` `j` | Move down |
| `Enter` `l` | Open run details |
| `←` `p` | Previous page |
| `→` `n` | Next page |
| `r` | Refresh |
| `R` | Re-run workflow |
| `C` | Cancel workflow |
| `o` | Open in browser |
| `q` | Quit |

### Run Details

| Key | Action |
|---|---|
| `↑` `k` | Navigate jobs |
| `↓` `j` | Navigate jobs |
| `Enter` `l` | View job logs |
| `Esc` `h` | Back to runs |
| `r` | Refresh |
| `o` | Open in browser |

### Log View

| Key | Action |
|---|---|
| `↑` `k` | Scroll up |
| `↓` `j` | Scroll down |
| `Esc` `h` | Back to details |

## Project Structure

```
src/
├── main.rs      # CLI, terminal setup, event loop
├── app.rs       # App state & navigation
├── ui.rs        # TUI rendering
├── github.rs    # GitHub REST API client
├── event.rs     # Key → action mapping
├── auth.rs      # Token resolution & OAuth device flow
└── models.rs    # WorkflowRun, Job, Step
```

## Options

```
atlas [OPTIONS] [COMMAND]

Options:
  -r, --repo <OWNER/REPO>   GitHub repository (default: auto-detect)
  -t, --token <TOKEN>        GitHub token (overrides stored credentials)
      --api-url <URL>        GitHub API base URL (for Enterprise)
  -v, --verbose              Debug logging to ~/.atlas/atlas.log
  -h, --help                 Print help
  -V, --version              Print version

Commands:
  auth login                 Authenticate via OAuth device flow
  auth logout                Remove stored credentials
  auth status                Show auth status
```

## License

MIT

---

<sub>Made by <a href="https://github.com/iliaselqadiri">Ilias El Qadiri</a></sub>

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};

use crate::app::{App, View};
use crate::models::Job;

// ── Color palette ──────────────────────────────────────────────────

const GREEN: Color = Color::Rgb(72, 199, 142);
const RED: Color = Color::Rgb(248, 81, 73);
const YELLOW: Color = Color::Rgb(210, 153, 34);
const BLUE: Color = Color::Rgb(88, 166, 255);
const PURPLE: Color = Color::Rgb(188, 140, 255);
const GRAY: Color = Color::Rgb(125, 133, 144);
const DIM: Color = Color::Rgb(48, 54, 61);
const BG: Color = Color::Rgb(13, 17, 23);
const FG: Color = Color::Rgb(230, 237, 243);
const HEADER_BG: Color = Color::Rgb(22, 27, 34);
const SELECTED_BG: Color = Color::Rgb(33, 38, 45);
const ORANGE: Color = Color::Rgb(210, 105, 30);

// ── Main draw entry point ──────────────────────────────────────────

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Fill background
    let bg_block = Block::default().style(Style::default().bg(BG));
    f.render_widget(bg_block, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status bar
            Constraint::Length(1), // Keybindings
        ])
        .split(size);

    draw_header(f, app, chunks[0]);

    match app.view {
        View::RepoList => draw_repo_list(f, app, chunks[1]),
        View::RunsList => draw_runs_list(f, app, chunks[1]),
        View::RunDetail => draw_run_detail(f, app, chunks[1]),
        View::Logs => draw_log_view(f, app, chunks[1]),
    }

    draw_status_bar(f, app, chunks[2]);
    draw_keybindings(f, app, chunks[3]);
}

// ── Header ─────────────────────────────────────────────────────────

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title_text = match app.view {
        View::RepoList => {
            let mut spans = vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "Atlas",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(DIM)),
                Span::styled(
                    "GitHub",
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(DIM)),
                Span::styled("Repositories", Style::default().fg(PURPLE)),
            ];
            if app.searching {
                spans.push(Span::styled(" │ ", Style::default().fg(DIM)));
                spans.push(Span::styled("🔍 ", Style::default()));
                spans.push(Span::styled(
                    &app.repo_filter,
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("▏", Style::default().fg(YELLOW)));
            }
            spans
        }
        _ => {
            vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "Atlas",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(DIM)),
                Span::styled(
                    "GitHub",
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(DIM)),
                Span::styled(
                    format!("{}/{}", app.client.owner, app.client.repo),
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(DIM)),
                Span::styled(
                    match app.view {
                        View::RunsList => "Workflow Runs",
                        View::RunDetail => "Run Details",
                        View::Logs => "Job Logs",
                        View::RepoList => unreachable!(),
                    },
                    Style::default().fg(PURPLE),
                ),
            ]
        }
    };

    let header = Paragraph::new(Line::from(title_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(HEADER_BG)),
    );

    f.render_widget(header, area);
}

// ── Repo List View ─────────────────────────────────────────────────

fn draw_repo_list(f: &mut Frame, app: &App, area: Rect) {
    let filtered = app.filtered_repos();

    if filtered.is_empty() {
        let msg = if app.loading {
            "  Loading repositories..."
        } else if !app.repo_filter.is_empty() {
            "  No repositories match your search."
        } else {
            "  No repositories found."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(GRAY).bg(BG))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(" Repositories ")
                    .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            );
        f.render_widget(p, area);
        return;
    }

    // Build table header
    let header_cells = [
        "",
        "🔒",
        "Repository",
        "Language",
        "Description",
        "Last Push",
        "⭐",
    ]
    .iter()
    .map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(GRAY)
                .add_modifier(Modifier::BOLD)
                .bg(HEADER_BG),
        )
    });
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, repo)| {
            let is_selected = i == app.repos_selected;
            let row_bg = if is_selected { SELECTED_BG } else { BG };

            let visibility_color = if repo.private { YELLOW } else { GREEN };
            let visibility = if repo.private { "🔒" } else { "🌍" };

            let lang_color = match repo.language.as_deref() {
                Some("Rust") => ORANGE,
                Some("TypeScript" | "JavaScript") => YELLOW,
                Some("Python") => BLUE,
                Some("Go") => Color::Rgb(0, 173, 216),
                Some("Java" | "Kotlin") => RED,
                Some("C" | "C++") => PURPLE,
                _ => GRAY,
            };

            let selector = if is_selected { "▸" } else { " " };
            let desc = repo
                .description
                .as_deref()
                .unwrap_or("—")
                .chars()
                .take(50)
                .collect::<String>();

            let stars = if repo.stargazers_count > 0 {
                repo.stargazers_count.to_string()
            } else {
                "—".to_string()
            };

            let cells = vec![
                Cell::from(selector).style(Style::default().fg(BLUE).bg(row_bg)),
                Cell::from(visibility).style(Style::default().fg(visibility_color).bg(row_bg)),
                Cell::from(repo.full_name.clone()).style(
                    Style::default()
                        .fg(FG)
                        .add_modifier(Modifier::BOLD)
                        .bg(row_bg),
                ),
                Cell::from(repo.language.as_deref().unwrap_or("—").to_string())
                    .style(Style::default().fg(lang_color).bg(row_bg)),
                Cell::from(desc).style(Style::default().fg(GRAY).bg(row_bg)),
                Cell::from(repo.last_active_display()).style(Style::default().fg(GRAY).bg(row_bg)),
                Cell::from(stars).style(Style::default().fg(YELLOW).bg(row_bg)),
            ];

            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(2),  // selector
        Constraint::Length(3),  // visibility
        Constraint::Min(20),    // full name
        Constraint::Length(14), // language
        Constraint::Min(20),    // description
        Constraint::Length(10), // last push
        Constraint::Length(5),  // stars
    ];

    let title = if app.repo_filter.is_empty() {
        format!(" Repositories ({}) ", app.repos.len())
    } else {
        format!(
            " Repositories ({}/{}) — \"{}\" ",
            filtered.len(),
            app.repos.len(),
            app.repo_filter
        )
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(title)
                .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(BG)),
        )
        .row_highlight_style(Style::default().bg(SELECTED_BG));

    let mut state = TableState::default();
    state.select(Some(app.repos_selected));
    f.render_stateful_widget(table, area, &mut state);

    // Scrollbar
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_style(Style::default().fg(DIM))
        .thumb_style(Style::default().fg(GRAY));
    let mut scrollbar_state = ScrollbarState::new(filtered.len()).position(app.repos_selected);
    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

// ── Runs List View ─────────────────────────────────────────────────

fn draw_runs_list(f: &mut Frame, app: &App, area: Rect) {
    if app.runs.is_empty() {
        let msg = if app.loading {
            "  Loading workflow runs..."
        } else {
            "No workflow runs found."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(GRAY).bg(BG))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(" Workflow Runs ")
                    .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            );
        f.render_widget(p, area);
        return;
    }

    // Build table header
    let header_cells = [
        "", "Status", "Workflow", "Branch", "Commit", "Event", "Duration", "Age", "Actor",
    ]
    .iter()
    .map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(GRAY)
                .add_modifier(Modifier::BOLD)
                .bg(HEADER_BG),
        )
    });
    let header = Row::new(header_cells).height(1);

    // Build table rows
    let rows: Vec<Row> = app
        .runs
        .iter()
        .enumerate()
        .map(|(i, run)| {
            let is_selected = i == app.runs_selected;
            let row_bg = if is_selected { SELECTED_BG } else { BG };

            let status_color = match run.conclusion.as_deref() {
                Some("success") => GREEN,
                Some("failure") => RED,
                Some("cancelled") => YELLOW,
                _ => match run.status.as_deref() {
                    Some("in_progress") => ORANGE,
                    Some("queued") => GRAY,
                    _ => GRAY,
                },
            };

            let icon = match run.conclusion.as_deref() {
                Some("success") => "✓",
                Some("failure") => "✗",
                Some("cancelled") => "⊘",
                _ => match run.status.as_deref() {
                    Some("in_progress") => "●",
                    Some("queued") => "◯",
                    _ => "?",
                },
            };

            let selector = if is_selected { "▸" } else { " " };

            let cells = vec![
                Cell::from(selector).style(Style::default().fg(BLUE).bg(row_bg)),
                Cell::from(format!("{} {}", icon, run.status_display()))
                    .style(Style::default().fg(status_color).bg(row_bg)),
                Cell::from(
                    run.display_title
                        .as_deref()
                        .or(run.name.as_deref())
                        .unwrap_or("—")
                        .to_string(),
                )
                .style(Style::default().fg(FG).bg(row_bg)),
                Cell::from(run.head_branch.as_deref().unwrap_or("—").to_string())
                    .style(Style::default().fg(PURPLE).bg(row_bg)),
                Cell::from(run.short_sha().to_string()).style(Style::default().fg(GRAY).bg(row_bg)),
                Cell::from(run.event.clone()).style(Style::default().fg(BLUE).bg(row_bg)),
                Cell::from(run.duration_display()).style(Style::default().fg(FG).bg(row_bg)),
                Cell::from(run.age_display()).style(Style::default().fg(GRAY).bg(row_bg)),
                Cell::from(
                    run.actor
                        .as_ref()
                        .map(|a| a.login.clone())
                        .unwrap_or_else(|| "—".to_string()),
                )
                .style(Style::default().fg(GRAY).bg(row_bg)),
            ];

            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(2),  // selector
        Constraint::Length(16), // status
        Constraint::Min(20),    // workflow name
        Constraint::Length(16), // branch
        Constraint::Length(9),  // commit
        Constraint::Length(12), // event
        Constraint::Length(10), // duration
        Constraint::Length(10), // age
        Constraint::Length(14), // actor
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(format!(" Workflow Runs ({}) ", app.runs_total))
                .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(BG)),
        )
        .row_highlight_style(Style::default().bg(SELECTED_BG));

    let mut state = TableState::default();
    state.select(Some(app.runs_selected));
    f.render_stateful_widget(table, area, &mut state);

    // Scrollbar
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_style(Style::default().fg(DIM))
        .thumb_style(Style::default().fg(GRAY));
    let mut scrollbar_state = ScrollbarState::new(app.runs.len()).position(app.runs_selected);
    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

// ── Run Detail View ────────────────────────────────────────────────

fn draw_run_detail(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Run summary
            Constraint::Min(8),    // Jobs + Steps
        ])
        .split(area);

    // ── Run summary box ────────────────────────────────────────────
    if let Some(run) = &app.current_run {
        let status_color = match run.conclusion.as_deref() {
            Some("success") => GREEN,
            Some("failure") => RED,
            Some("cancelled") => YELLOW,
            _ => ORANGE,
        };

        let summary_lines = vec![
            Line::from(vec![
                Span::styled("  Run #", Style::default().fg(GRAY)),
                Span::styled(
                    run.run_number.to_string(),
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled(run.status_display(), Style::default().fg(status_color)),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled(&run.event, Style::default().fg(BLUE)),
                Span::styled(" on ", Style::default().fg(GRAY)),
                Span::styled(
                    run.head_branch.as_deref().unwrap_or("—"),
                    Style::default().fg(PURPLE),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    run.display_title
                        .as_deref()
                        .or(run.name.as_deref())
                        .unwrap_or("—"),
                    Style::default().fg(FG),
                ),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled(run.short_sha(), Style::default().fg(GRAY)),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled(run.duration_display(), Style::default().fg(FG)),
                Span::styled(" · ", Style::default().fg(DIM)),
                Span::styled(
                    run.actor.as_ref().map(|a| a.login.as_str()).unwrap_or("—"),
                    Style::default().fg(GRAY),
                ),
            ]),
        ];

        let summary = Paragraph::new(summary_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(status_color))
                .title(" Run Summary ")
                .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(HEADER_BG)),
        );
        f.render_widget(summary, chunks[0]);
    }

    // ── Jobs & Steps ───────────────────────────────────────────────
    if app.jobs.is_empty() {
        let msg = if app.loading {
            "⏳ Loading jobs..."
        } else {
            "No jobs found for this run."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(GRAY).bg(BG))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM))
                    .title(" Jobs ")
                    .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            );
        f.render_widget(p, chunks[1]);
        return;
    }

    // Split into jobs list and steps panel
    let detail_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Jobs
            Constraint::Percentage(60), // Steps
        ])
        .split(chunks[1]);

    // Jobs list
    draw_jobs_list(f, app, detail_chunks[0]);

    // Steps for selected job
    if let Some(job) = app.jobs.get(app.jobs_selected) {
        draw_steps(f, job, detail_chunks[1]);
    }
}

fn draw_jobs_list(f: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let is_selected = i == app.jobs_selected;
            let row_bg = if is_selected { SELECTED_BG } else { BG };

            let status_color = match job.conclusion.as_deref() {
                Some("success") => GREEN,
                Some("failure") => RED,
                Some("cancelled") => YELLOW,
                _ => ORANGE,
            };

            let icon = match job.conclusion.as_deref() {
                Some("success") => "✓",
                Some("failure") => "✗",
                Some("cancelled") => "⊘",
                _ => "●",
            };

            let selector = if is_selected { "▸" } else { " " };

            let cells = vec![
                Cell::from(selector).style(Style::default().fg(BLUE).bg(row_bg)),
                Cell::from(icon.to_string()).style(Style::default().fg(status_color).bg(row_bg)),
                Cell::from(job.name.clone()).style(Style::default().fg(FG).bg(row_bg)),
                Cell::from(job.duration_display()).style(Style::default().fg(GRAY).bg(row_bg)),
            ];

            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(format!(" Jobs ({}) ", app.jobs.len()))
                .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(BG)),
        )
        .row_highlight_style(Style::default().bg(SELECTED_BG));

    let mut state = TableState::default();
    state.select(Some(app.jobs_selected));
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_steps(f: &mut Frame, job: &Job, area: Rect) {
    let steps = job.steps.as_deref().unwrap_or(&[]);

    let lines: Vec<Line> = steps
        .iter()
        .map(|step| {
            let status_color = match step.conclusion.as_deref() {
                Some("success") => GREEN,
                Some("failure") => RED,
                Some("cancelled") => YELLOW,
                Some("skipped") => GRAY,
                _ => ORANGE,
            };

            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(step.status_icon(), Style::default().fg(status_color)),
                Span::styled("  ", Style::default()),
                Span::styled(&step.name, Style::default().fg(FG)),
                Span::styled("  ", Style::default()),
                Span::styled(step.duration_display(), Style::default().fg(GRAY)),
            ])
        })
        .collect();

    let status_color = match job.conclusion.as_deref() {
        Some("success") => GREEN,
        Some("failure") => RED,
        Some("cancelled") => YELLOW,
        _ => ORANGE,
    };

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .title(format!(
                " {} · {} · {} ",
                job.name,
                job.status_display(),
                job.duration_display()
            ))
            .title_style(Style::default().fg(status_color))
            .padding(Padding::vertical(1))
            .style(Style::default().bg(BG)),
    );

    f.render_widget(p, area);
}

// ── Log View ───────────────────────────────────────────────────────

fn draw_log_view(f: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = app
        .log_content
        .iter()
        .map(|line| {
            let color = if line.contains("##[error]") || line.contains("Error") {
                RED
            } else if line.contains("##[warning]") || line.contains("Warning") {
                YELLOW
            } else if line.contains("##[group]") || line.starts_with("Run ") {
                BLUE
            } else {
                FG
            };
            Line::from(Span::styled(line.as_str(), Style::default().fg(color)))
        })
        .collect();

    let title = if let Some(job) = app.jobs.get(app.jobs_selected) {
        format!(" Logs: {} ({} lines) ", job.name, app.log_content.len())
    } else {
        " Logs ".to_string()
    };

    let p = Paragraph::new(lines)
        .scroll(((app.log_scroll.min(u16::MAX as usize)) as u16, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM))
                .title(title)
                .title_style(Style::default().fg(FG).add_modifier(Modifier::BOLD))
                .padding(Padding::horizontal(1))
                .style(Style::default().bg(BG)),
        );

    f.render_widget(p, area);

    // Scrollbar for logs
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"))
        .track_style(Style::default().fg(DIM))
        .thumb_style(Style::default().fg(GRAY));
    let total = app.log_content.len();
    let mut scrollbar_state = ScrollbarState::new(total).position(app.log_scroll);
    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

// ── Status bar ─────────────────────────────────────────────────────

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let loading_indicator = if app.loading { "⏳ " } else { "" };

    let status = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(loading_indicator, Style::default().fg(YELLOW)),
        Span::styled(&app.status_message, Style::default().fg(FG)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(DIM))
            .style(Style::default().bg(HEADER_BG)),
    );

    f.render_widget(status, area);
}

// ── Keybindings bar ────────────────────────────────────────────────

fn draw_keybindings(f: &mut Frame, app: &App, area: Rect) {
    let bindings = match app.view {
        View::RepoList => {
            if app.searching {
                vec![
                    ("type", "filter"),
                    ("Esc", "clear"),
                    ("↑↓", "navigate"),
                    ("Enter", "open"),
                    ("q", "quit"),
                ]
            } else {
                vec![
                    ("↑↓/jk", "navigate"),
                    ("Enter/l", "open"),
                    ("/", "search"),
                    ("r", "refresh"),
                    ("o", "browser"),
                    ("q", "quit"),
                ]
            }
        }
        View::RunsList => vec![
            ("↑↓/jk", "navigate"),
            ("Enter/l", "open"),
            ("r", "refresh"),
            ("←→/np", "page"),
            ("o", "browser"),
            ("R", "rerun"),
            ("C", "cancel"),
            ("q", "quit"),
        ],
        View::RunDetail => vec![
            ("↑↓/jk", "navigate"),
            ("Enter/l", "logs"),
            ("Esc/h", "back"),
            ("r", "refresh"),
            ("o", "browser"),
            ("R", "rerun"),
            ("C", "cancel"),
            ("q", "quit"),
        ],
        View::Logs => vec![
            ("↑↓/jk", "scroll"),
            ("Esc/h", "back"),
            ("r", "refresh"),
            ("o", "browser"),
            ("q", "quit"),
        ],
    };

    let spans: Vec<Span> = bindings
        .iter()
        .enumerate()
        .flat_map(|(i, (key, desc))| {
            let mut v = vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default()
                        .fg(BG)
                        .bg(GRAY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {} ", desc), Style::default().fg(GRAY)),
            ];
            if i < bindings.len() - 1 {
                v.push(Span::styled("│", Style::default().fg(DIM)));
            }
            v
        })
        .collect();

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG));
    f.render_widget(bar, area);
}

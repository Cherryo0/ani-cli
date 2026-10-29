//! ratatui rendering — MovieBox-Tui visual language (Catppuccin Mocha),
//! ani-cli branding. Same look, different name.

use crate::app::{App, Screen};
use crate::model::{Anime, SourceKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

// ── Catppuccin Mocha — MovieBox-Tui's default theme ──────────────────────────
const BORDER: Color = Color::Rgb(88, 91, 112);
const BORDER_FOCUS: Color = Color::Rgb(137, 180, 250);
const TEXT: Color = Color::Rgb(205, 214, 244);
const TEXT_DIM: Color = Color::Rgb(166, 173, 200);
const TITLE: Color = Color::Rgb(203, 166, 247); // mauve
const HIGHLIGHT: Color = Color::Rgb(137, 180, 250); // blue
const SHORTCUT: Color = Color::Rgb(250, 179, 135); // peach
const TEAL: Color = Color::Rgb(148, 226, 213);
const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
const ERROR: Color = Color::Rgb(243, 139, 168);

fn st(c: Color) -> Style {
    Style::default().fg(c)
}
fn bold(c: Color) -> Style {
    Style::default().fg(c).add_modifier(Modifier::BOLD)
}

const LOGO: &str = r#" █████╗  ███╗  ██╗ ██╗         ██████╗ ██╗      ██╗
██╔══██╗ ████╗ ██║ ██║        ██╔════╝ ██║      ██║
███████║ ██╔██╗██║ ██║        ██║      ██║      ██║
██╔══██║ ██║╚████║ ██║        ██║      ██║      ██║
██║  ██║ ██║ ╚███║ ██║        ╚██████╗ ███████╗ ██║
╚═╝  ╚═╝ ╚═╝  ╚══╝ ╚═╝         ╚═════╝ ╚══════╝ ╚═╝"#;
const LOGO_WIDTH: u16 = 55;

pub fn render(f: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::Home => render_home(f, app),
        Screen::Detail => render_detail(f, app, f.area()),
        Screen::Sources => render_sources(f, app, f.area()),
        Screen::Help => {
            render_home(f, app);
            render_help_popup(f);
        }
    }
}

// ── shared bits ─────────────────────────────────────────────────────────────

/// Centered bottom bar: `[key]` peach + dim label, `·` separators.
/// Mirrors MovieBox-Tui's `home_bottom_bar_spans`.
fn bottom_bar(f: &mut Frame, area: Rect, items: &[(&str, &str)]) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", st(TEXT_DIM)));
        }
        spans.push(Span::styled("[", st(TEXT_DIM)));
        spans.push(Span::styled(*key, st(SHORTCUT)));
        spans.push(Span::styled("]", st(TEXT_DIM)));
        spans.push(Span::styled(format!(" {label}"), st(TEXT_DIM)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn centered(width: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let x = area.x + area.width.saturating_sub(w) / 2;
    Rect { x, width: w, ..area }
}

// ── home ────────────────────────────────────────────────────────────────────

fn render_home(f: &mut Frame, app: &mut App) {
    // Landing (logo) while there is nothing to show yet — like MovieBox-Tui.
    if app.items.is_empty() && !app.is_search_results {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(6),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(f.area());
        render_logo(f, chunks[1]);
        render_version(f, chunks[2]);
        render_search_bar(f, app, centered(76, chunks[4]));
        render_center_status(f, app, chunks[5]);
        bottom_bar(
            f,
            chunks[6],
            &[
                ("/", "search"),
                ("tab", "provider"),
                ("enter", "open"),
                ("p", "player"),
                ("?", "help"),
                ("q", "quit"),
            ],
        );
        return;
    }

    // Results view.
    let mut constraints = vec![Constraint::Length(3)];
    let show_status = app.loading || !app.status.is_empty();
    if show_status {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    render_search_bar(f, app, chunks[0]);
    let mut idx = 1;
    if show_status {
        render_status_line(f, app, chunks[1]);
        idx = 2;
    }
    render_results_list(f, app, chunks[idx]);
    bottom_bar(
        f,
        chunks[idx + 1],
        &[
            ("/", "search"),
            ("tab", "provider"),
            ("enter", "open"),
            ("p", "player"),
            ("?", "help"),
            ("q", "quit"),
        ],
    );
}

fn render_logo(f: &mut Frame, area: Rect) {
    if area.width < LOGO_WIDTH + 4 {
        f.render_widget(
            Paragraph::new("ANI-CLI")
                .alignment(Alignment::Center)
                .style(bold(TITLE)),
            area,
        );
        return;
    }
    let x = area.x + area.width.saturating_sub(LOGO_WIDTH) / 2;
    let logo_area = Rect {
        x,
        width: LOGO_WIDTH,
        ..area
    };
    f.render_widget(Paragraph::new(LOGO).style(bold(TITLE)), logo_area);
}

fn render_version(f: &mut Frame, area: Rect) {
    let x = area.x + area.width.saturating_sub(LOGO_WIDTH) / 2;
    let version_area = Rect {
        x,
        width: LOGO_WIDTH.min(area.width),
        ..area
    };
    f.render_widget(
        Paragraph::new(format!("v{}", env!("CARGO_PKG_VERSION")))
            .alignment(Alignment::Right)
            .style(st(TEXT_DIM)),
        version_area,
    );
}

/// Search bar with the provider pill on the right — `[HiAnimeX · Tab]`,
/// mirroring MovieBox-Tui's `[Provider · Ctrl+P]` pill.
fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(st(if app.input_mode {
            BORDER_FOCUS
        } else {
            BORDER
        }));
    f.render_widget(block, area);

    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: 1,
    };
    if inner.width == 0 {
        return;
    }

    let provider = &app.providers[app.provider_idx];
    let pill = format!("[{} · Tab]", provider.name());
    let pill_width = pill.len() as u16;

    let query = if app.search_input.is_empty() && !app.input_mode {
        Span::styled("Search anime…", st(TEXT_DIM))
    } else {
        Span::styled(format!("❯ {}", app.search_input), st(TEXT))
    };
    let left_width = inner.width.saturating_sub(pill_width + 1);
    f.render_widget(Paragraph::new(Line::from(query)), Rect {
        width: left_width,
        ..inner
    });
    f.render_widget(
        Paragraph::new(Span::styled(pill, bold(SAPPHIRE))).alignment(Alignment::Right),
        Rect {
            x: inner.x + left_width,
            width: pill_width + 1,
            ..inner
        },
    );

    if app.input_mode {
        // Clamp the cursor so a very long query can't push it off-screen.
        let cx = (inner.x + 2 + app.search_input.len() as u16)
            .min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((cx, inner.y));
    }
}

fn render_center_status(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 {
        return;
    }
    if app.loading {
        f.render_widget(
            Paragraph::new("⏳ Loading…")
                .alignment(Alignment::Center)
                .style(st(HIGHLIGHT)),
            area,
        );
    } else if !app.status.is_empty() {
        let color = if app.status.starts_with("Error") {
            ERROR
        } else {
            TEXT_DIM
        };
        f.render_widget(
            Paragraph::new(app.status.clone())
                .alignment(Alignment::Center)
                .style(st(color)),
            area,
        );
    }
}

fn render_status_line(f: &mut Frame, app: &App, area: Rect) {
    if app.loading {
        f.render_widget(
            Paragraph::new("⏳ Loading…")
                .alignment(Alignment::Center)
                .style(st(HIGHLIGHT)),
            area,
        );
    } else {
        let color = if app.status.starts_with("Error") {
            ERROR
        } else {
            TEXT_DIM
        };
        f.render_widget(
            Paragraph::new(app.status.clone())
                .alignment(Alignment::Center)
                .style(st(color)),
            area,
        );
    }
}

fn render_results_list(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let heading = if app.is_search_results {
        "Results"
    } else {
        "Trending"
    };
    let left = format!("{heading} — {} titles", app.items.len());
    let right = format!("{} · p", app.player.name());
    let pad = chunks[0]
        .width
        .saturating_sub(left.len() as u16 + right.len() as u16) as usize;
    let header = Line::from(vec![
        Span::styled(left, st(TEXT_DIM)),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, st(TEXT_DIM)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    if app.items.is_empty() && !app.loading {
        f.render_widget(
            Paragraph::new("No results — try another search")
                .alignment(Alignment::Center)
                .style(st(TEXT_DIM)),
            chunks[1],
        );
        return;
    }

    let items: Vec<ListItem> = app.items.iter().map(|a| ListItem::new(anime_line(a))).collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(45, 45, 60))
                .fg(HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);
}

fn anime_line(a: &Anime) -> Line<'static> {
    let mut spans = vec![Span::styled(a.title.clone(), st(TEXT))];
    if !a.languages.is_empty() {
        spans.push(Span::raw("  "));
        for (i, lang) in a.languages.iter().enumerate() {
            let sep = if i + 1 < a.languages.len() { ", " } else { "" };
            spans.push(Span::styled(format!("{lang}{sep}"), st(TEAL)));
        }
    }
    if !a.note.is_empty() {
        spans.push(Span::styled(format!("  · {}", a.note), st(TEXT_DIM)));
    }
    Line::from(spans)
}

// ── detail ──────────────────────────────────────────────────────────────────

fn render_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(detail) = app.detail.clone() else { return };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Meta block — languages shown BEFORE the episode list, as requested.
    let a = &detail.anime;
    let langs = if a.languages.is_empty() {
        "unknown".to_string()
    } else {
        a.languages.join(" · ")
    };
    let meta = vec![
        Line::from(vec![Span::styled(a.title.clone(), bold(TITLE))]),
        Line::from(vec![
            Span::styled("Languages: ", st(TEXT_DIM)),
            Span::styled(langs, bold(TEAL)),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "{}  {}  {}",
                detail.status,
                detail.year,
                detail.genres.join(", ")
            ),
            st(TEXT_DIM),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            truncate(&detail.synopsis, 400),
            st(TEXT_DIM),
        )]),
    ];
    f.render_widget(
        Paragraph::new(meta).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Details ")
                .border_style(st(BORDER)),
        ),
        chunks[0],
    );

    let ep_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(chunks[1]);
    f.render_widget(
        Paragraph::new(format!("Episodes — {} total", app.episodes.len())).style(st(TEXT_DIM)),
        ep_chunks[0],
    );
    let items: Vec<ListItem> = app
        .episodes
        .iter()
        .map(|e| {
            let langs = if e.languages.is_empty() {
                String::new()
            } else {
                format!("  [{}]", e.languages.join(", "))
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("EP {} ", e.number), bold(HIGHLIGHT)),
                Span::styled(e.title.clone(), st(TEXT)),
                Span::styled(langs, st(TEAL)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(45, 45, 60))
                .fg(HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, ep_chunks[1], &mut app.ep_state);

    bottom_bar(
        f,
        chunks[2],
        &[
            ("enter", "play episode"),
            ("b", "back"),
            ("p", "player"),
            ("q", "quit"),
        ],
    );
}

// ── sources ─────────────────────────────────────────────────────────────────

fn render_sources(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let title = app
        .current_episode_title()
        .unwrap_or_else(|| "Sources".to_string());
    f.render_widget(
        Paragraph::new(title).style(bold(TITLE)),
        chunks[0],
    );

    let items: Vec<ListItem> = app
        .sources
        .iter()
        .map(|s| {
            let kind = match s.kind {
                SourceKind::Direct => "file",
                SourceKind::Hls => "hls",
                SourceKind::EmbedPage => "browser",
            };
            let q = s.quality.clone().unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(s.label.clone(), bold(TEXT)),
                Span::styled(format!("  [{kind}]"), st(TEXT_DIM)),
                Span::styled(format!("  {q}"), st(TEAL)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(45, 45, 60))
                .fg(HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.src_state);

    bottom_bar(f, chunks[2], &[("enter", "play"), ("b", "back"), ("q", "quit")]);
}

// ── help ────────────────────────────────────────────────────────────────────

fn render_help_popup(f: &mut Frame) {
    let area = centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);
    let key = |k: &str| Span::styled(format!("  {k}"), st(SHORTCUT));
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(" keybindings", bold(TITLE))]),
        Line::from(""),
        Line::from(vec![key("/"), Span::styled("            focus search", st(TEXT))]),
        Line::from(vec![key("enter"), Span::styled("        open / play", st(TEXT))]),
        Line::from(vec![key("tab"), Span::styled("          switch provider", st(TEXT))]),
        Line::from(vec![key("p"), Span::styled("            switch video player", st(TEXT))]),
        Line::from(vec![key("b / esc"), Span::styled("      back", st(TEXT))]),
        Line::from(vec![key("j / k"), Span::styled("          move (also ↑ ↓)", st(TEXT))]),
        Line::from(vec![key("?"), Span::styled("            this help", st(TEXT))]),
        Line::from(vec![key("q"), Span::styled("            quit", st(TEXT))]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Language tags like [Hindi, English] are parsed from",
            st(TEXT_DIM),
        )]),
        Line::from(vec![Span::styled(
            "  the release title and shown before you pick an episode.",
            st(TEXT_DIM),
        )]),
    ];
    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Help ")
                .border_style(st(BORDER_FOCUS)),
        ),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup[1])[1]
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

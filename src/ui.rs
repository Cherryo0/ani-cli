//! ratatui rendering — MovieBox-Tui-flavoured screens.

use crate::app::{App, Screen};
use ani_cli::model::{Anime, SourceKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub const ACCENT: Color = Color::Rgb(198, 242, 78); // lime
pub const MUTED: Color = Color::DarkGray;

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);

    match app.screen {
        Screen::Home => render_home(f, app, chunks[1]),
        Screen::Detail => render_detail(f, app, chunks[1]),
        Screen::Sources => render_sources(f, app, chunks[1]),
        Screen::Help => {
            render_home(f, app, chunks[1]);
            render_help_popup(f);
        }
    }

    render_footer(f, app, chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let providers: Vec<Span> = app
        .providers
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            let active = i == app.provider_idx;
            let style = if active {
                Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            };
            let sep = if i + 1 < app.providers.len() {
                vec![Span::raw("  ")]
            } else {
                vec![]
            };
            std::iter::once(Span::styled(format!(" {} ", p.name()), style)).chain(sep)
        })
        .collect();

    let mut lines = vec![Line::from(vec![
        Span::styled(
            " ani-cli ",
            Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ])];
    lines[0].spans.extend(providers);
    lines.push(Line::from(vec![
        Span::styled(
            format!(" player: {} ", app.player.name()),
            Style::default().fg(ACCENT),
        ),
        Span::styled(" (p to change)", Style::default().fg(MUTED)),
    ]));

    let header = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(MUTED)),
    );
    f.render_widget(header, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let keys = match app.screen {
        Screen::Home => "/ search · tab provider · enter open · p player · ? help · q quit",
        Screen::Detail => "enter play episode · b back · p player · q quit",
        Screen::Sources => "enter play · b back · q quit",
        Screen::Help => "any key close",
    };
    let status = if app.loading {
        Span::styled(" ⏳ loading…", Style::default().fg(ACCENT))
    } else if app.status.is_empty() {
        Span::raw("")
    } else {
        Span::styled(format!(" {}", app.status), Style::default().fg(Color::Yellow))
    };
    let footer = Paragraph::new(vec![
        Line::from(vec![Span::styled(keys, Style::default().fg(MUTED))]),
        Line::from(vec![status]),
    ])
    .alignment(Alignment::Left)
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)),
    );
    f.render_widget(footer, area);
}

fn render_home(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search box
    let prompt = if app.input_mode { "❯ " } else { "/ " };
    let search = Paragraph::new(format!("{}{}", prompt, app.search_input)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Search (Enter to search, Esc to unfocus) ")
            .border_style(Style::default().fg(if app.input_mode { ACCENT } else { MUTED })),
    );
    f.render_widget(search, chunks[0]);
    if app.input_mode {
        f.set_cursor_position((
            chunks[0].x + prompt.len() as u16 + app.search_input.len() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // Results / trending list
    let title = if app.is_search_results { " Results " } else { " Trending " };
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|a| ListItem::new(anime_line(a)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{title} — {} titles ", app.items.len()))
                .border_style(Style::default().fg(MUTED)),
        )
        .highlight_style(
            Style::default().bg(Color::Rgb(40, 44, 30)).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);
}

fn anime_line(a: &Anime) -> Line<'static> {
    let mut spans = vec![Span::raw(a.title.clone())];
    if !a.languages.is_empty() {
        spans.push(Span::raw("  "));
        for (i, lang) in a.languages.iter().enumerate() {
            let sep = if i + 1 < a.languages.len() { ", " } else { "" };
            spans.push(Span::styled(
                format!("{lang}{sep}"),
                Style::default().fg(ACCENT),
            ));
        }
    }
    if !a.note.is_empty() {
        spans.push(Span::styled(
            format!("  · {}", a.note),
            Style::default().fg(MUTED),
        ));
    }
    Line::from(spans)
}

fn render_detail(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(detail) = app.detail.clone() else { return };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    // Meta block — languages are shown BEFORE the episode list, as requested.
    let a = &detail.anime;
    let langs = if a.languages.is_empty() {
        "unknown".to_string()
    } else {
        a.languages.join(" · ")
    };
    let meta = vec![
        Line::from(vec![Span::styled(
            a.title.clone(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("Languages: ", Style::default().fg(MUTED)),
            Span::styled(langs, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}  {}  {}", detail.status, detail.year, detail.genres.join(", ")),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            truncate(&detail.synopsis, 400),
            Style::default().fg(Color::Gray),
        )]),
    ];
    f.render_widget(
        Paragraph::new(meta)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title(" Details ")),
        chunks[0],
    );

    // Episodes
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
                Span::styled(format!("EP {} ", e.number), Style::default().fg(ACCENT)),
                Span::raw(e.title.clone()),
                Span::styled(langs, Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Episodes ({} total) ", app.episodes.len())),
        )
        .highlight_style(Style::default().bg(Color::Rgb(40, 44, 30)).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.ep_state);
}

fn render_sources(f: &mut Frame, app: &mut App, area: Rect) {
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
                Span::styled(s.label.clone(), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format!("  [{kind}]"), Style::default().fg(MUTED)),
                Span::styled(format!("  {q}"), Style::default().fg(ACCENT)),
            ]))
        })
        .collect();
    let title = app
        .current_episode_title()
        .unwrap_or_else(|| "Sources".to_string());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} — pick a server ")),
        )
        .highlight_style(Style::default().bg(Color::Rgb(40, 44, 30)).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, &mut app.src_state);
}

fn render_help_popup(f: &mut Frame) {
    let area = centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(" keybindings", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from("  /            focus search"),
        Line::from("  enter        open / play"),
        Line::from("  tab          switch provider"),
        Line::from("  p            switch video player"),
        Line::from("  b / esc      back"),
        Line::from("  j / k        move (also ↑ ↓)"),
        Line::from("  ?            this help"),
        Line::from("  q            quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Language tags like [Hindi, English] are parsed from",
            Style::default().fg(MUTED),
        )]),
        Line::from(vec![Span::styled(
            "  the release title and shown before you pick an episode.",
            Style::default().fg(MUTED),
        )]),
    ];
    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .border_style(Style::default().fg(ACCENT)),
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

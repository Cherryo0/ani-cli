//! App state machine + event loop.

use crate::{
    model::{Anime, AnimeDetails, Episode, StreamSource},
    player::{self, PlayerKind},
    providers::{self, Provider},
};
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use std::{io::Stdout, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Detail,
    Sources,
    Help,
}

enum Pending {
    Trending,
    Search(String),
    Detail(usize),
    Sources(usize),
}

pub enum LoopAction {
    Quit,
    Play { source: StreamSource, title: String },
}

pub struct App {
    pub providers: Vec<Box<dyn Provider>>,
    pub provider_idx: usize,
    pub player: PlayerKind,
    pub screen: Screen,
    pub input_mode: bool,
    pub search_input: String,
    pub items: Vec<Anime>,
    pub is_search_results: bool,
    pub list_state: ListState,
    pub detail: Option<AnimeDetails>,
    pub episodes: Vec<Episode>,
    pub ep_state: ListState,
    pub sources: Vec<StreamSource>,
    pub src_state: ListState,
    pub status: String,
    pub loading: bool,
    history: Vec<Screen>,
    pending: Option<Pending>,
    selected_episode: Option<Episode>,
}

impl App {
    pub fn new(player_override: Option<String>, provider_override: Option<String>) -> Self {
        let providers = providers::all_providers();
        let provider_idx = provider_override
            .as_deref()
            .and_then(|id| providers.iter().position(|p| p.id() == id))
            .unwrap_or(0);
        let player = player_override
            .as_deref()
            .and_then(PlayerKind::from_name)
            .or_else(player::detect)
            .unwrap_or(PlayerKind::Mpv);
        let status = if player::is_available(player) {
            String::new()
        } else {
            format!("{} not found on PATH — pick another with 'p'", player.name())
        };
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            providers,
            provider_idx,
            player,
            screen: Screen::Home,
            input_mode: false,
            search_input: String::new(),
            items: Vec::new(),
            is_search_results: false,
            list_state,
            detail: None,
            episodes: Vec::new(),
            ep_state: ListState::default(),
            sources: Vec::new(),
            src_state: ListState::default(),
            status,
            loading: false,
            history: Vec::new(),
            pending: None,
            selected_episode: None,
        }
    }

    fn push_screen(&mut self, next: Screen) {
        let cur = std::mem::replace(&mut self.screen, next);
        self.history.push(cur);
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.screen = prev;
            self.status.clear();
        }
    }

    fn cycle_provider(&mut self) {
        self.provider_idx = (self.provider_idx + 1) % self.providers.len();
        self.is_search_results = false;
        self.search_input.clear();
        self.pending = Some(Pending::Trending);
    }

    fn cycle_player(&mut self) {
        let all = PlayerKind::all();
        let i = all.iter().position(|p| *p == self.player).unwrap_or(0);
        self.player = all[(i + 1) % all.len()];
        self.status = if player::is_available(self.player) {
            format!("Player: {}", self.player.name())
        } else {
            format!("{} not installed", self.player.name())
        };
    }

    fn move_sel(&mut self, dir: i32) {
        let (state, len) = match self.screen {
            Screen::Home => (&mut self.list_state, self.items.len()),
            Screen::Detail => (&mut self.ep_state, self.episodes.len()),
            Screen::Sources => (&mut self.src_state, self.sources.len()),
            Screen::Help => return,
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as i32;
        state.select(Some(cur.wrapping_add(dir).rem_euclid(len as i32) as usize));
    }

    fn page_sel(&mut self, dir: i32) {
        let (state, len) = match self.screen {
            Screen::Home => (&mut self.list_state, self.items.len()),
            Screen::Detail => (&mut self.ep_state, self.episodes.len()),
            Screen::Sources => (&mut self.src_state, self.sources.len()),
            Screen::Help => return,
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as i32;
        let next = (cur + dir).clamp(0, len as i32 - 1);
        state.select(Some(next as usize));
    }

    fn jump_sel(&mut self, end: bool) {
        let (state, len) = match self.screen {
            Screen::Home => (&mut self.list_state, self.items.len()),
            Screen::Detail => (&mut self.ep_state, self.episodes.len()),
            Screen::Sources => (&mut self.src_state, self.sources.len()),
            Screen::Help => return,
        };
        if len == 0 {
            return;
        }
        state.select(Some(if end { len - 1 } else { 0 }));
    }

    pub fn current_episode_title(&self) -> Option<String> {
        let ep = self.selected_episode.as_ref()?;
        let anime = self
            .detail
            .as_ref()
            .map(|d| d.anime.title.clone())
            .unwrap_or_default();
        Some(format!("{anime} — EP {}", ep.number))
    }

    // -- async work ---------------------------------------------------------

    async fn execute_pending(&mut self, p: Pending) {
        let result: Result<()> = async {
            match p {
                Pending::Trending => {
                    let items = self.providers[self.provider_idx].trending().await?;
                    self.items = items;
                    self.is_search_results = false;
                    self.list_state.select(Some(0));
                    self.status.clear();
                }
                Pending::Search(q) => {
                    let items = self.providers[self.provider_idx].search(&q).await?;
                    let n = items.len();
                    self.items = items;
                    self.is_search_results = true;
                    self.list_state.select(Some(0));
                    self.status = format!("{n} result(s) for \"{q}\"");
                }
                Pending::Detail(i) => {
                    if let Some(anime) = self.items.get(i).cloned() {
                        let details = self.providers[self.provider_idx].details(&anime).await?;
                        let episodes = self.providers[self.provider_idx].episodes(&anime).await?;
                        self.push_screen(Screen::Detail);
                        self.detail = Some(details);
                        self.episodes = episodes;
                        self.ep_state.select(Some(0));
                        self.status.clear();
                    }
                }
                Pending::Sources(i) => {
                    if let Some(ep) = self.episodes.get(i).cloned() {
                        let sources = self.providers[self.provider_idx].sources(&ep).await?;
                        if sources.is_empty() {
                            anyhow::bail!("No playable sources found for this episode");
                        }
                        self.push_screen(Screen::Sources);
                        self.sources = sources;
                        self.src_state.select(Some(0));
                        self.selected_episode = Some(ep);
                        self.status.clear();
                    }
                }
            }
            Ok(())
        }
        .await;
        if let Err(e) = result {
            self.status = format!("Error: {e:#}");
        }
    }

    // -- input --------------------------------------------------------------

    /// Key handling mirrors MovieBox-Tui: Ctrl+P cycles the provider,
    /// typing on Home starts a search, Enter/Space opens, j/k/arrows move,
    /// PgUp/PgDn/Home/End jump, Esc/q backs out, Ctrl+C quits.
    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> Option<LoopAction> {
        if mods.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') | KeyCode::Char('C') => return Some(LoopAction::Quit),
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.cycle_provider();
                    return None;
                }
                KeyCode::Char('u') | KeyCode::Char('U') if self.input_mode => {
                    self.search_input.clear();
                    return None;
                }
                KeyCode::Char('w') | KeyCode::Char('W') if self.input_mode => {
                    while self.search_input.ends_with(' ') {
                        self.search_input.pop();
                    }
                    while !self.search_input.is_empty()
                        && !self.search_input.ends_with(' ')
                    {
                        self.search_input.pop();
                    }
                    return None;
                }
                _ => {}
            }
        }
        if self.screen == Screen::Help {
            if matches!(
                code,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Char('?')
            ) {
                self.go_back();
            }
            return None;
        }
        if self.input_mode {
            match code {
                KeyCode::Esc => self.input_mode = false,
                KeyCode::Enter => {
                    self.input_mode = false;
                    let q = self.search_input.trim().to_string();
                    if !q.is_empty() {
                        self.pending = Some(Pending::Search(q));
                    }
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                }
                KeyCode::Char(c) => self.search_input.push(c),
                _ => {}
            }
            return None;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Some(LoopAction::Quit),
            KeyCode::Char('?') => self.push_screen(Screen::Help),
            KeyCode::Char('p') => self.cycle_player(),
            KeyCode::Char('b') | KeyCode::Char('B') => self.go_back(),
            KeyCode::Esc => {
                if self.screen == Screen::Home {
                    return Some(LoopAction::Quit);
                }
                self.go_back();
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.screen {
                Screen::Home => {
                    if let Some(i) = self.list_state.selected() {
                        self.pending = Some(Pending::Detail(i));
                    }
                }
                Screen::Detail => {
                    if let Some(i) = self.ep_state.selected() {
                        self.pending = Some(Pending::Sources(i));
                    }
                }
                Screen::Sources => {
                    if let Some(i) = self.src_state.selected() {
                        if let Some(source) = self.sources.get(i).cloned() {
                            let title = self
                                .current_episode_title()
                                .unwrap_or_else(|| "ani-cli".to_string());
                            return Some(LoopAction::Play { source, title });
                        }
                    }
                }
                Screen::Help => {}
            },
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => self.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => self.move_sel(1),
            KeyCode::PageUp => self.page_sel(-10),
            KeyCode::PageDown => self.page_sel(10),
            KeyCode::Home => self.jump_sel(false),
            KeyCode::End => self.jump_sel(true),
            // MovieBox-Tui: typing anywhere on Home starts a search.
            KeyCode::Char(c) if self.screen == Screen::Home => {
                self.input_mode = true;
                if c == '/' {
                    self.search_input.clear();
                } else {
                    self.search_input.push(c);
                }
            }
            KeyCode::Backspace if self.screen == Screen::Home => {
                self.input_mode = true;
            }
            _ => {}
        }
        None
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<LoopAction> {
        loop {
            terminal.draw(|f| ui::render(f, self))?;
            if let Some(p) = self.pending.take() {
                self.loading = true;
                terminal.draw(|f| ui::render(f, self))?;
                self.execute_pending(p).await;
                self.loading = false;
                continue;
            }
            if event::poll(Duration::from_millis(120))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if let Some(action) = self.handle_key(key.code, key.modifiers) {
                        return Ok(action);
                    }
                }
            }
        }
    }
}

/// Set up the terminal, run the UI until the user quits or plays something,
/// then tear the terminal back down.
async fn run_ui(app: &mut App) -> Result<LoopAction> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let action = app.event_loop(&mut terminal).await;
    // Always restore the terminal, even when the event loop errored —
    // otherwise the user's shell is left in raw mode on a dead screen.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    action
}

pub async fn run(player_override: Option<String>, provider_override: Option<String>) -> Result<()> {
    let mut app = App::new(player_override, provider_override);
    app.pending = Some(Pending::Trending);
    loop {
        match run_ui(&mut app).await? {
            LoopAction::Quit => break,
            LoopAction::Play { source, title } => {
                if let Err(e) = player::play(app.player, &source, &title) {
                    app.status = format!("Player error: {e:#}");
                } else {
                    app.status = format!("Finished: {title}");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod keymap_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key(code: KeyCode) -> (KeyCode, KeyModifiers) {
        (code, KeyModifiers::empty())
    }
    fn ctrl(code: KeyCode) -> (KeyCode, KeyModifiers) {
        (code, KeyModifiers::CONTROL)
    }

    fn app_with_items(n: usize) -> App {
        let mut app = App::new(None, None);
        app.items = (0..n)
            .map(|i| crate::model::Anime {
                id: i.to_string(),
                title: format!("Anime {i}"),
                url: String::new(),
                poster: None,
                languages: vec![],
                note: String::new(),
                provider_id: "hianimex".into(),
            })
            .collect();
        app
    }

    #[test]
    fn ctrl_p_cycles_provider_like_moviebox() {
        let mut app = App::new(None, None);
        let first = app.provider_idx;
        let (code, mods) = ctrl(KeyCode::Char('p'));
        app.handle_key(code, mods);
        assert_ne!(app.provider_idx, first, "Ctrl+P must switch provider");
    }

    #[test]
    fn typing_on_home_starts_search_like_moviebox() {
        let mut app = App::new(None, None);
        let (code, mods) = key(KeyCode::Char('n'));
        app.handle_key(code, mods);
        assert!(app.input_mode, "typing must focus search");
        assert_eq!(app.search_input, "n");
    }

    #[test]
    fn slash_clears_and_focuses_search() {
        let mut app = App::new(None, None);
        app.search_input = "old".into();
        let (code, mods) = key(KeyCode::Char('/'));
        app.handle_key(code, mods);
        assert!(app.input_mode);
        assert_eq!(app.search_input, "", "slash must clear the old query");
    }

    #[test]
    fn space_opens_like_enter() {
        let mut app = app_with_items(3);
        let (code, mods) = key(KeyCode::Char(' '));
        let action = app.handle_key(code, mods);
        assert!(action.is_none());
        assert!(matches!(app.pending, Some(Pending::Detail(0))));
    }

    #[test]
    fn page_down_jumps_ten_rows_clamped() {
        let mut app = app_with_items(25);
        let (code, mods) = key(KeyCode::PageDown);
        app.handle_key(code, mods);
        assert_eq!(app.list_state.selected(), Some(10));
        app.handle_key(code, mods);
        app.handle_key(code, mods);
        assert_eq!(app.list_state.selected(), Some(24), "must clamp at the end");
    }

    #[test]
    fn home_end_jump_to_ends() {
        let mut app = app_with_items(25);
        let (code, mods) = key(KeyCode::End);
        app.handle_key(code, mods);
        assert_eq!(app.list_state.selected(), Some(24));
        let (code, mods) = key(KeyCode::Home);
        app.handle_key(code, mods);
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn ctrl_u_clears_search_input() {
        let mut app = App::new(None, None);
        app.input_mode = true;
        app.search_input = "naruto".into();
        let (code, mods) = ctrl(KeyCode::Char('u'));
        app.handle_key(code, mods);
        assert_eq!(app.search_input, "");
        assert!(app.input_mode, "must stay in editing");
    }

    #[test]
    fn ctrl_w_deletes_word_backwards() {
        let mut app = App::new(None, None);
        app.input_mode = true;
        app.search_input = "one piece".into();
        let (code, mods) = ctrl(KeyCode::Char('w'));
        app.handle_key(code, mods);
        assert_eq!(app.search_input, "one "); // standard readline Ctrl+W keeps the separator
    }

    #[test]
    fn help_opens_and_closes_like_moviebox() {
        let mut app = App::new(None, None);
        let (code, mods) = key(KeyCode::Char('?'));
        app.handle_key(code, mods);
        assert_eq!(app.screen, Screen::Help);
        let (code, mods) = key(KeyCode::Esc);
        app.handle_key(code, mods);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn esc_in_search_unfocuses_without_searching() {
        let mut app = App::new(None, None);
        app.input_mode = true;
        app.search_input = "bleach".into();
        let (code, mods) = key(KeyCode::Esc);
        app.handle_key(code, mods);
        assert!(!app.input_mode);
        assert!(app.pending.is_none(), "Esc must not trigger a search");
    }
}

//! App state machine + event loop.

use crate::{
    history,
    model::{Anime, AnimeDetails, Episode, HistoryEntry, StreamSource},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeView {
    /// Recently played episodes (default).
    History,
    /// Full-text search results.
    Search,
    /// Trending titles, via the `/trending` command.
    Trending,
}

enum Pending {
    Search(String),
    Trending,
    Detail(usize),
    HistoryDetail(usize),
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
    pub home_view: HomeView,
    pub list_state: ListState,
    pub detail: Option<AnimeDetails>,
    pub episodes: Vec<Episode>,
    pub ep_state: ListState,
    pub sources: Vec<StreamSource>,
    pub src_state: ListState,
    pub status: String,
    pub loading: bool,
    /// Recently played episodes — shown on the home screen instead of trending.
    pub watch_history: Vec<HistoryEntry>,
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
        let watch_history = history::load_history();
        let items: Vec<Anime> = watch_history.iter().map(|e| e.as_anime()).collect();
        Self {
            providers,
            provider_idx,
            player,
            screen: Screen::Home,
            input_mode: false,
            search_input: String::new(),
            items,
            home_view: HomeView::History,
            list_state,
            detail: None,
            episodes: Vec::new(),
            ep_state: ListState::default(),
            sources: Vec::new(),
            src_state: ListState::default(),
            status,
            loading: false,
            watch_history,
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
        self.search_input.clear();
        self.refresh_home_items();
        self.status.clear();
    }

    /// Home list always mirrors the watch history (no trending).
    fn refresh_home_items(&mut self) {
        self.items = self.watch_history.iter().map(|e| e.as_anime()).collect();
        self.home_view = HomeView::History;
        self.list_state.select(Some(0));
    }

    /// Record the just-played episode at the front of the history
    /// (deduplicated) and persist it.
    fn record_play(&mut self) {
        let (Some(detail), Some(ep)) =
            (self.detail.clone(), self.selected_episode.clone())
        else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let entry = HistoryEntry {
            anime_id: detail.anime.id.clone(),
            anime_title: detail.anime.title.clone(),
            anime_url: detail.anime.url.clone(),
            provider_id: self.providers[self.provider_idx].id().to_string(),
            episode_number: ep.number.clone(),
            episode_title: ep.title.clone(),
            played_at_unix: now,
        };
        self.watch_history.retain(|e| {
            !(e.anime_id == entry.anime_id
                && e.episode_number == entry.episode_number
                && e.provider_id == entry.provider_id)
        });
        self.watch_history.insert(0, entry);
        self.watch_history.truncate(50);
        history::save_history(&self.watch_history);
        self.refresh_home_items();
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
                Pending::Search(q) => {
                    let items = self.providers[self.provider_idx].search(&q).await?;
                    let n = items.len();
                    self.items = items;
                    self.home_view = HomeView::Search;
                    self.list_state.select(Some(0));
                    self.status = format!("{n} result(s) for \"{q}\"");
                }
                Pending::Trending => {
                    let items = self.providers[self.provider_idx].trending().await?;
                    let n = items.len();
                    self.items = items;
                    self.home_view = HomeView::Trending;
                    self.list_state.select(Some(0));
                    self.status = format!("{n} trending titles");
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
                Pending::HistoryDetail(i) => {
                    if let Some(entry) = self.watch_history.get(i).cloned() {
                        if let Some(pi) = self
                            .providers
                            .iter()
                            .position(|p| p.id() == entry.provider_id)
                        {
                            self.provider_idx = pi;
                        }
                        let anime = entry.as_anime();
                        let details =
                            self.providers[self.provider_idx].details(&anime).await?;
                        let episodes =
                            self.providers[self.provider_idx].episodes(&anime).await?;
                        self.push_screen(Screen::Detail);
                        self.detail = Some(details);
                        self.episodes = episodes;
                        // Pre-select the episode that was played.
                        let sel = self
                            .episodes
                            .iter()
                            .position(|e| e.number == entry.episode_number)
                            .unwrap_or(0);
                        self.ep_state.select(Some(sel));
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
                    // Slash commands, MovieBox-Tui style.
                    if q.eq_ignore_ascii_case("/trending") {
                        self.search_input.clear();
                        self.pending = Some(Pending::Trending);
                    } else if q.eq_ignore_ascii_case("/history") {
                        self.search_input.clear();
                        self.refresh_home_items();
                        self.status.clear();
                    } else if !q.is_empty() {
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
            // MovieBox-Tui: q backs out everywhere except Home, where it quits.
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if self.screen == Screen::Home {
                    return Some(LoopAction::Quit);
                }
                self.go_back();
            }
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
                        // Search results and trending open normally;
                        // history entries reopen their anime's detail page.
                        self.pending = Some(match self.home_view {
                            HomeView::History => Pending::HistoryDetail(i),
                            HomeView::Search | HomeView::Trending => Pending::Detail(i),
                        });
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
    loop {
        match run_ui(&mut app).await? {
            LoopAction::Quit => break,
            LoopAction::Play { source, title } => {
                // Record first so the episode is in history even if
                // the player fails to launch.
                app.record_play();
                if let Err(e) = player::play(app.player, &source, &title) {
                    app.status = format!("Player error: {e:#}");
                } else {
                    app.status = format!("Finished: {title}");
                }
                // Loop back into the TUI — the app never quits on play.
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
        app.home_view = HomeView::Search;
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

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::model::{AnimeDetails, Episode};
    use crossterm::event::{KeyCode, KeyModifiers};

    static ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> =
        std::sync::OnceLock::new();

    /// Run `f` with HOME pointed at a fresh temp dir so history
    /// persistence never touches real user data.
    fn with_temp_home(f: impl FnOnce()) {
        let _guard = ENV_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap();
        let old = std::env::var("HOME").ok();
        let tmp = std::env::temp_dir().join(format!("ani-cli-home-{}", std::process::id()));
        std::env::set_var("HOME", &tmp);
        f();
        match old {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn key(code: KeyCode) -> (KeyCode, KeyModifiers) {
        (code, KeyModifiers::empty())
    }

    fn entry(ep: &str) -> HistoryEntry {
        HistoryEntry {
            anime_id: "43".into(),
            anime_title: "Naruto".into(),
            anime_url: "https://example.com/n".into(),
            provider_id: "hianimex".into(),
            episode_number: ep.into(),
            episode_title: format!("Episode {ep}"),
            played_at_unix: 1,
        }
    }

    fn played_app(ep_number: &str, ep_title: &str) -> App {
        let mut app = App::new(None, None);
        app.detail = Some(AnimeDetails {
            anime: crate::model::Anime {
                id: "43".into(),
                title: "Naruto".into(),
                url: "https://example.com/n".into(),
                poster: None,
                languages: vec![],
                note: String::new(),
                provider_id: "hianimex".into(),
            },
            synopsis: String::new(),
            genres: vec![],
            status: String::new(),
            year: String::new(),
        });
        app.selected_episode = Some(Episode {
            number: ep_number.into(),
            title: ep_title.into(),
            url: String::new(),
            languages: vec![],
            servers: vec![],
        });
        app
    }

    #[test]
    fn q_on_detail_goes_back_instead_of_quitting() {
        let mut app = App::new(None, None);
        app.push_screen(Screen::Detail);
        let (code, mods) = key(KeyCode::Char('q'));
        let action = app.handle_key(code, mods);
        assert!(action.is_none(), "q on Detail must not quit the app");
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn q_on_home_still_quits() {
        let mut app = App::new(None, None);
        let (code, mods) = key(KeyCode::Char('q'));
        let action = app.handle_key(code, mods);
        assert!(matches!(action, Some(LoopAction::Quit)));
    }

    #[test]
    fn enter_on_history_item_opens_history_detail() {
        let mut app = App::new(None, None);
        app.watch_history = vec![entry("1")];
        app.refresh_home_items();
        assert_eq!(app.home_view, HomeView::History);
        let (code, mods) = key(KeyCode::Enter);
        app.handle_key(code, mods);
        assert!(matches!(app.pending, Some(Pending::HistoryDetail(0))));
    }

    #[test]
    fn record_play_prepends_dedupes_and_persists() {
        with_temp_home(|| {
            let mut app = played_app("2", "Episode 2");
            app.watch_history = vec![entry("1")];
            app.record_play();
            assert_eq!(app.watch_history.len(), 2);
            assert_eq!(app.watch_history[0].episode_number, "2");

            // Replaying EP1 moves it to the front without duplicating.
            let mut app2 = played_app("1", "Episode 1");
            app2.watch_history = app.watch_history.clone();
            app2.record_play();
            assert_eq!(app2.watch_history.len(), 2);
            assert_eq!(app2.watch_history[0].episode_number, "1");

            // Persisted to disk.
            let loaded = history::load_history();
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].episode_number, "1");
        });
    }

    #[test]
    fn fresh_app_shows_history_not_trending() {
        with_temp_home(|| {
            history::save_history(&[entry("7")]);
            let app = App::new(None, None);
            assert_eq!(app.home_view, HomeView::History);
            assert_eq!(app.items.len(), 1);
            assert_eq!(app.items[0].title, "Naruto");
            assert!(app.pending.is_none(), "no auto-trending on boot");
        });
    }
}

#[cfg(test)]
mod slash_command_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn enter_with_input(input: &str) -> App {
        let mut app = App::new(None, None);
        app.input_mode = true;
        app.search_input = input.into();
        let mods = KeyModifiers::empty();
        app.handle_key(KeyCode::Enter, mods);
        app
    }

    #[test]
    fn slash_trending_queues_trending_fetch() {
        let app = enter_with_input("/trending");
        assert!(!app.input_mode);
        assert_eq!(app.search_input, "");
        assert!(matches!(app.pending, Some(Pending::Trending)));
    }

    #[test]
    fn slash_trending_is_case_insensitive() {
        let app = enter_with_input("/TRENDING");
        assert!(matches!(app.pending, Some(Pending::Trending)));
    }

    #[test]
    fn slash_history_returns_to_history_view() {
        let mut app = App::new(None, None);
        app.home_view = HomeView::Trending;
        app.input_mode = true;
        app.search_input = "/history".into();
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(app.home_view, HomeView::History);
        assert!(app.pending.is_none(), "/history must not fetch anything");
    }

    #[test]
    fn normal_search_still_queues_search() {
        let app = enter_with_input("naruto");
        assert!(matches!(app.pending, Some(Pending::Search(_))));
    }
}

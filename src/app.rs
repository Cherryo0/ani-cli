//! App state machine + event loop.

use ani_cli::{
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
    fn new(player_override: Option<String>, provider_override: Option<String>) -> Self {
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

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) -> Option<LoopAction> {
        if mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c')) {
            return Some(LoopAction::Quit);
        }
        if self.screen == Screen::Help {
            self.go_back();
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
            KeyCode::Char('q') => return Some(LoopAction::Quit),
            KeyCode::Char('?') => self.push_screen(Screen::Help),
            KeyCode::Char('p') => self.cycle_player(),
            KeyCode::Char('b') => self.go_back(),
            KeyCode::Esc => {
                if self.screen == Screen::Home {
                    return Some(LoopAction::Quit);
                }
                self.go_back();
            }
            KeyCode::Tab => {
                if self.screen == Screen::Home {
                    self.cycle_provider();
                }
            }
            KeyCode::Char('/') => {
                if self.screen == Screen::Home {
                    self.input_mode = true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Enter => match self.screen {
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
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
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

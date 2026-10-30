# app.rs — state machine + event loop

The heart of the TUI. Owns all mutable state, the keymap, the async
network operations (`Pending`), and the terminal event loop.

## Enums

### `Screen`
Which screen is showing: `Home | Detail | Sources | Help`.
Navigation stack in `App.history: Vec<Screen>` (`push_screen` / `go_back`).

### `HomeView`
What the home list is showing (v0.1.5+; home never auto-fetches trending):

- `History` — recently played episodes from `history.json` (default).
- `Search` — full-text search results.
- `Trending` — trending titles, only via the `/trending` slash command.

### `Pending` (private)
One async network op queued by a keypress, executed by `execute_pending`:

- `Search(String)` — provider search.
- `Trending` — provider trending fetch.
- `Detail(usize)` — open search/trending result `i` (details + episodes).
- `HistoryDetail(usize)` — reopen history entry `i`: switches to that
  entry's provider, opens its detail page, pre-selects the played episode.
- `Sources(usize)` — fetch sources for episode `i`.

### `LoopAction` (public)
What `run_ui` returns to `run()`:

- `Quit` — exit the app.
- `Play { source, title }` — hand this source to `player::play`, then
  **return to the TUI** (v0.1.5+: the app never quits after playback).

## `App` struct

Public fields: `providers`, `provider_idx`, `player: PlayerKind`,
`screen`, `input_mode`, `search_input`, `items: Vec<Anime>`,
`home_view`, `list_state`, `detail: Option<AnimeDetails>`,
`episodes: Vec<Episode>`, `ep_state`, `sources: Vec<StreamSource>`,
`src_state`, `status: String`, `loading: bool`,
`watch_history: Vec<HistoryEntry>`.
Private: `history` (screen stack), `pending`, `selected_episode`.

## Methods

- `App::new(player_override, provider_override)` — builds providers, picks
  player (override → auto-detect → mpv), loads history into `items`,
  warns in `status` if the player binary is missing.
- `push_screen(next)` / `go_back()` — screen stack navigation.
- `cycle_provider()` — Ctrl+P: next provider, clear search, home back to
  history view.
- `refresh_home_items()` — rebuild `items` from `watch_history`,
  `home_view = History`.
- `record_play()` — **before** playback: prepend/refresh the
  `HistoryEntry` (dedupe by anime+episode+provider, most-recent-first,
  cap 50), persist via `history::save_history`. Runs even if the player
  fails, so history is never lost.
- `cycle_player()` — `p` key: rotate through available players.
- `move_sel(dir)` — j/k/arrows, ±1 clamped.
- `page_sel(dir)` — PgUp/PgDn, ±10 clamped.
- `jump_sel(end)` — Home/End.
- `current_episode_title()` — "Title EP n" for the player window title.
- `execute_pending(p)` (async) — runs the queued network op:
  `Search` → `home_view = Search`; `Trending` → `home_view = Trending`;
  `Detail` → fetch details + episodes, push Detail screen;
  `HistoryDetail` → provider switch + detail + episode pre-select;
  `Sources` → fetch sources, push Sources screen. Errors land in `status`.
- `handle_key(code, mods) -> Option<LoopAction>` — the full keymap (below).
- `event_loop(terminal, app)` (async) — crossterm event loop; on
  `LoopAction::Play` calls `record_play()` then returns the action.
- `run_ui(app)` (async) — sets up raw mode + alternate screen, renders,
  runs the loop, **restores the terminal even on error** (v0.1.1 bugfix).
- `run(player_override, provider_override)` (async, public) — builds the
  app, loops `run_ui` → `player::play` → back to TUI until `Quit`.

## Keymap (MovieBox-Tui parity)

| Key | Action |
|-----|--------|
| Ctrl+C | quit (anywhere) |
| Ctrl+P | cycle provider |
| q / Esc | back (Detail/Sources/Help); **quit only on Home** |
| b | back |
| ? | help popup (Esc/q/? closes) |
| p | cycle video player |
| Enter / Space | select (Home → detail, Detail → sources, Sources → play) |
| j/k, ↑/↓ | move ±1 |
| PgUp/PgDn | move ±10 |
| Home/End | jump to ends |
| type on Home | start searching (`/` focuses with cleared input) |
| Ctrl+U / Ctrl+W | clear input / delete word (in search) |
| Esc (in search) | unfocus without searching |

Slash commands (typed in the search box + Enter):

- `/trending` — fetch trending titles into the home list.
- `/history` — back to the recently-played list (no network).

## Tests (in-file, `cargo test`)

- `keymap_tests` (10): Ctrl+P cycling, type-to-search, `/` focus,
  Space==Enter, PgDn ±10 clamp, Home/End, Ctrl+U, Ctrl+W, help open/close,
  Esc-unfocus.
- `history_tests` (5): `q` backs out on Detail but quits on Home, Enter on
  history item → `HistoryDetail`, `record_play` prepend/dedupe/persist,
  fresh app shows history (never trending).
- `slash_command_tests` (4): `/trending` queues fetch (case-insensitive),
  `/history` returns to history view without fetching, normal search
  unaffected.
- History tests run under `with_temp_home` (temp `HOME`) so they never
  touch real user data.

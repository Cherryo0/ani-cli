# ui.rs — rendering

All drawing. Catppuccin Mocha theme, MovieBox-Tui layout parity:
block-letter logo, centered rounded search bar with provider pill, peach
`[key]` bottom-bar hints, dim body text.

## Palette (Catppuccin Mocha)

- Titles: mauve (203,166,247) bold · headers: pink (245,194,231) ·
  highlight/selected: blue (137,180,250) bold · shortcuts: peach
  (250,179,135) · body: dim (166,173,200) · borders: (88,91,112).
- `st(c) -> Style`, `bold(c) -> Style` (private helpers).

## Render functions

- `render(f, app)` (public) — dispatches on `app.screen`; draws the
  bottom bar per screen.
- `bottom_bar(f, area, items)` (private) — `[key] label · [key] label`
  hints, peach keys + dim labels.
- `render_home` — logo + version when the history list is empty
  (landing); otherwise the heading (`Recently played` / `Results` /
  `Trending`, from `HomeView`) + `render_results_list`.
- `render_logo` / `render_version` — big ASCII `ANI-CLI` + version,
  right-aligned like MovieBox-Tui.
- `render_search_bar` — centered rounded box, provider pill
  `[HiAnimeX · Ctrl+P]`, cursor when `input_mode`.
- `render_center_status` — loading spinner / error / empty-list messages
  ("No recently played anime yet — search to start watching").
- `render_status_line` — one-line status (result counts, errors).
- `render_results_list` — borderless list; rows via `anime_line`
  (title + `[lang, …]` badges + note).
- `render_detail` — pink title header, **Languages line before episodes**,
  synopsis, genres, episode list.
- `render_sources` — server picker list with `[hls]`/`[src]` kind badges.
- `render_help_popup` — centered popup: full keymap + `/trending` and
  `/history` slash commands.
- `centered`, `centered_rect`, `truncate` (private layout helpers).

## Tests

`tests/ux_render.rs` — 10 TestBackend render tests: home landing, history
rows, search results with language badges, detail screen, sources screen,
help popup, footer hints per screen. A mutation test proved they catch
real regressions (renaming "Languages:" / changing `[hls]` fails exactly
the right tests).
`examples/preview.rs` — renders every screen to text for visual checks
without a terminal.

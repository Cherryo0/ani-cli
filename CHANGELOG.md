# CHANGELOG — ani-cli

Recent work, newest first. Each entry is a tagged release on
`Cherryo0/ani-cli` with the Homebrew formula bumped to match.

## v0.1.7 — HiAnimeAt + subtitles (2026-10-30)

The user asked to add pystardust/ani-cli's anime servers and to verify
playback end-to-end.

- **New provider `HiAnimeAt`** (`src/providers/hianimeat.rs`) — the
  hianime.at backend pystardust/ani-cli v5.x uses (same HiAnime network,
  newer `/api/theme/` endpoints). Its servers API returns a base64
  `data-hash` that decodes straight to the embed URL — one fewer request
  than HiAnimeX. ZokoAnime embeds → direct HLS; others → browser fallback.
  Provider order is now HiAnimeX → HiAnimeAt → WatchAnimeWorld →
  AnimoStream.
- **Subtitles on direct streams** — the ZokoAnime payload's default
  subtitle track is extracted (`extractors::parse_subtitle_url`) and
  passed to the player: mpv `--sub-file`, vlc `:input-slave`, iina
  `--mpv-sub-files`. `StreamSource.subtitle_url` added for this.
- **End-to-end playback proof** — new live test
  `hianimeat_full_chain_with_subtitles`: search → episodes → sources,
  asserts the first source is in-player kind, a subtitle URL exists, and
  the m3u8 serves a valid `#EXTM3U` playlist. All 5 live tests pass.

## v0.1.6 — `/trending` and `/history` slash commands (2026-10-29)

MovieBox-Tui-style slash commands typed in the search box:

- `/trending` fetches trending titles on demand; `/history` jumps back
  to recently-played without a network call (case-insensitive).
- `App.home_view` is now an enum (`History | Search | Trending`)
  replacing the old `is_search_results` bool; headings follow the view.
- Help popup documents both commands. 4 new unit tests.

## v0.1.5 — don't quit after play, history home (2026-10-29)

Two user complaints fixed:

- **No trending on home** — the auto-fetched trending list is gone. Home
  shows **Recently played** from `~/.config/ani-cli/history.json`
  (dedupe by anime+episode+provider, most-recent-first, cap 50). Enter on
  a history item reopens that anime's detail page with the played episode
  pre-selected, switching to the right provider automatically.
- **Don't quit after playing** — root cause was `q` quitting from every
  screen. Now `q`/`Esc`/`b` go back on Detail/Sources and quit only on
  Home; the run loop returns to the TUI after the player exits.
- `record_play()` runs **before** playback so history survives player
  failures. History starts empty (nothing was recorded before this).

## v0.1.4 — Direct sources sort first (2026-10-29)

The user asked why playback opened the browser: Direct (mpv-playable)
sources were 3rd/6th in the list, so the default pick hit encrypted
MegaPlay embeds. `sort_sources_for_playback` puts Direct/HLS above
`EmbedPage` fallbacks — the default pick now plays in mpv.

## v0.1.3 — MovieBox-Tui keymap parity (2026-10-29)

Exact keys from MovieBox-Tui's real code: Ctrl+P provider cycling,
type-to-search on Home, Space select, PgUp/PgDn/Home/End, Ctrl+U/W in
search, Esc/q/? closing help. 10 new keymap unit tests (36 total).

## v0.1.2 — MovieBox-Tui restyle (2026-10-29)

Full visual rebuild against MovieBox-Tui's actual UI code after the user
said it didn't look right: Catppuccin Mocha palette, block-letter logo,
centered search bar with provider pill, peach key hints. The lime/tab-bar
design was removed.

## v0.1.1 — TUI UX audit (2026-10-29)

Custom `tui-ux-audit` skill applied: 10 TestBackend render tests,
heuristic score 17/20 → rated 8/10. Found and fixed a real bug —
terminal (raw mode/alternate screen) not restored when the event loop
errored.

## v0.1.0 — initial release (2026-10-29)

Rust TUI anime app: 3 providers (HiAnimeX with cracked ZokoAnime direct
HLS via the `otaku-embed-v1` XOR deobfuscation, WatchAnimeWorld,
AnimoStream), 6 video players (mpv default), language badges on every
row, per-episode language line before watching.

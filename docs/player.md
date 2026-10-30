# player.rs — video players

Six players, mpv first. Detects what's installed, builds the right
command line per player, and falls back to the browser for `EmbedPage`
sources.

## `PlayerKind` (public enum)

`Mpv | Vlc | Iina | Ffplay | Celluloid | Haruna`, in preference order.
`name()` → `"mpv"` etc.; `from_name(s)` — case-insensitive parse for the
CLI `--player` flag; `all()` — the ordered list.

## Functions

- `is_available(kind) -> bool` — binary on PATH (`which_binary`).
- `detect() -> Option<PlayerKind>` — first available in preference order.
- `build_command(player, source: &StreamSource, title: &str) -> Result<Command>`
  — the per-player command line:
  - **mpv**: `--force-media-title`, Referer via `--http-header-fields`,
    `--hls-bitrate=max` for HLS, `--sub-file=<url>` when subtitles exist,
    `--no-terminal`.
  - **vlc**: `--http-referrer`, `:input-slave=<sub>` for subtitles,
    `--meta-title`, `--play-and-exit`.
  - **iina**: `--no-stdin --keep-running`, subtitles via
    `--mpv-sub-files` (colons escaped — mpv splits sub-files on `:`),
    `--mpv-referrer`, `--mpv-force-media-title`.
  - **ffplay**: `-autoexit -window_title`, Referer via `-headers`.
  - **celluloid**: mpv-style `--http-header-fields` Referer.
  - **haruna**: bare URL.
- `play(player, source, title) -> Result<()>` — if `SourceKind::EmbedPage`,
  opens the URL in the system browser (`open` on macOS, `xdg-open`
  elsewhere) instead of a player; otherwise spawns the built command and
  blocks until the player exits.
- `open_in_browser(url)` (private) — the `open`/`xdg-open` dispatch.

## Tests

- `mpv_gets_referer_header`, `mpv_gets_sub_file_when_subtitles_present`,
  `mpv_omits_sub_file_without_subtitles`, `from_name_roundtrip`.

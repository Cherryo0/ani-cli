# history.rs — watch-history persistence

Tiny JSON store at `~/.config/ani-cli/history.json`. This is what the
home screen shows ("Recently played") instead of auto-fetched trending.

## Functions

- `history_path() -> Option<PathBuf>` (private) — `$HOME/.config/ani-cli/history.json`.
- `load_history() -> Vec<HistoryEntry>` — empty vec if missing/unreadable.
- `save_history(entries: &[HistoryEntry])` — pretty JSON, creates parent
  dirs; silently no-ops without `HOME`.
- `load_history_from(path)` / `save_history_to(path, entries)` — the
  testable cores (used with temp paths in tests).

## Rules (enforced in `App::record_play`, not here)

Most-recent-first, deduped by anime+episode+provider, capped at 50.
Recorded **before** playback starts, so an entry survives even if the
player fails.

## Tests

`history_roundtrips_through_disk`, `missing_file_loads_empty`.

# docs — ani-cli module reference

Every module, every public type, every function. Start here, then dive in.

| Doc | Module | What it covers |
|-----|--------|----------------|
| [app.md](app.md) | `src/app.rs` (831 lines) | State machine: screens, HomeView, keymap, pending ops, event loop, watch-history recording, all unit tests |
| [ui.md](ui.md) | `src/ui.rs` (572 lines) | Rendering: Catppuccin Mocha theme, logo, search bar, lists, detail/sources/help screens |
| [providers.md](providers.md) | `src/providers/` (5 files) | `Provider` trait, all 4 providers, language-tag parsing, shared HTTP client |
| [extractors.md](extractors.md) | `src/extractors.rs` | ZokoAnime `otaku-embed-v1` deobfuscation → direct HLS + subtitles |
| [player.md](player.md) | `src/player.rs` | Player detection, per-player command building, subtitle args, browser fallback |
| [model.md](model.md) | `src/model.rs` | `Anime`, `Episode`, `StreamSource`, `SourceKind`, `HistoryEntry` |
| [history.md](history.md) | `src/history.rs` | `~/.config/ani-cli/history.json` persistence |
| [main.md](main.md) | `src/main.rs`, `src/lib.rs`, `tests/`, `examples/` | CLI entry, crate layout, live smoke tests, render tests, preview example |

Cross-cutting flows:

- **Search → watch:** `App::handle_key` (Enter on Home) → `Pending::Search`
  → `Provider::search` → `Pending::Detail` → `Provider::details` →
  `Provider::episodes` → `Pending::Sources` → `Provider::sources` →
  `LoopAction::Play` → `player::play`.
- **Direct-stream upgrade:** provider finds a `zokoanime.video` embed →
  `extractors::resolve_zokoanime` → `SourceKind::Hls` + `subtitle_url` →
  sorted first → mpv, never the browser.
- **History:** `App::record_play` runs *before* playback → `history.json`
  → home screen shows `HistoryEntry::as_anime()` rows.

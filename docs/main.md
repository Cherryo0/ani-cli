# main.rs / lib.rs / tests / examples

## `src/main.rs` — CLI entry

clap `Cli`: `--player/-p` (mpv, vlc, iina, ffplay, celluloid, haruna;
auto-detected if omitted), `--provider/-P` (hianimex, hianimeat,
watchanimeworld, animostream). `main` → `ani_cli::app::run(player, provider)`.

## `src/lib.rs` — crate root

`pub mod app, extractors, history, model, player, providers, ui;`
Shared by the binary and the integration tests.

## `tests/live_smoke.rs` — live tests (ignored by default)

`cargo test -- --ignored` hits the real sites (~10s):

1. `hianimex_search_and_episodes` — search + episode list.
2. `hianimex_server_chain` — sources; prints every label → URL.
3. `zokoanime_extractor_yields_direct_hls` — extractor → `.m3u8`.
4. `watchanimeworld_search` — search + language tags.
5. `hianimeat_full_chain_with_subtitles` — **the end-to-end playback
   proof**: search → episodes → sources; first source must be in-player
   kind (`Hls`/`Direct`), a subtitle URL must be present, and the m3u8
   must serve a valid `#EXTM3U` playlist over HTTP.

## `tests/ux_render.rs` — render tests

10 TestBackend tests (see [ui.md](ui.md)); run offline with `cargo test`.

## `examples/preview.rs`

Renders every screen as plain text (`cargo run --example preview`) for
eyeballing the UI without launching the TUI.

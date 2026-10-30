# providers/ — anime sources

One file per site, all behind the `Provider` trait. The UI only talks to
the trait, so adding a site = one new file + one line in
`all_providers()`.

## `mod.rs` — shared pieces

### `trait Provider: Send + Sync`
- `id() -> &'static str` — e.g. `"hianimex"`.
- `name() -> &'static str` — display name, e.g. `"HiAnimeX"`.
- `base_url() -> &'static str`.
- `trending()`, `search(query)`, `details(anime)`, `episodes(anime)`,
  `sources(episode)` — all `async`, all `anyhow::Result`.

### `all_providers() -> Vec<Box<dyn Provider>>`
Priority order (Ctrl+P cycles): **HiAnimeX → HiAnimeAt →
WatchAnimeWorld → AnimoStream**. The two HiAnime providers come first
because they yield *direct* HLS streams (in-player); the others are
`EmbedPage` (browser fallback).

### `provider_by_id(id) -> Option<Box<dyn Provider>>`
Lookup used by the CLI `--provider` flag, history re-entry, and tests.

### Language-tag parsing
- `LANG_RE` — regex matching language tokens (hindi, eng, tamil, jap,
  "dual audio", …) inside release titles.
- `normalise_language(token) -> String` — `"eng"` → `"English"`,
  `"jap"` → `"Japanese"`, else title-case.
- `parse_languages(title) -> Vec<String>` — ordered, de-duplicated tags.
  Shown on every row **before** the user picks an episode (the user's core
  requirement).

### `http_client() -> reqwest::Client` (public)
Shared client: real browser UA (sites 403 library UAs), 25s timeout, and
`SSL_CERT_FILE` PEM roots honoured on top of webpki — required on networks
that MITM TLS. **Always use this; never `reqwest::Client::new()`.**

## `hianimex.rs` — HiAnimeX (default provider)

`https://hianimex.com.in`, id `hianimex`. Classic v2 ajax API:

- `search`: `/search?keyword=<q>` → `.flw-item` cards, SUB/DUB tick
  badges → language tags.
- `trending`: `/home` cards, truncated to 24.
- `episodes`: `/ajax/v2/episode/list/<animeId>` → JSON `html` with
  `.ep-item` `data-number`/`data-id`; raw episode id stashed in
  `ServerRef.url` for `sources()`.
- `sources`: `/ajax/v2/episode/servers?episodeId=` → per `data-type`
  (sub/dub) server `data-id`s → `/ajax/v2/episode/sources?id=` → embed
  links. **`zokoanime.video` embeds are resolved via
  `extractors::resolve_zokoanime` into direct HLS + subtitles** (the
  browser-opening bug the user reported was fixed by sorting these first).
  `sort_sources_for_playback` puts Direct/HLS above EmbedPage.

## `hianimeat.rs` — HiAnimeAt (pystardust/ani-cli's backend)

`https://hianime.at`, id `hianimeat`. Added v0.1.7 at the user's request —
this is the backend pystardust/ani-cli v5.x uses. Same HiAnime network,
newer `/api/theme/` endpoints:

- `search`: `/search?keyword=<q>` (sidebar markup cut off first).
- `episodes`: `/api/theme/episode/list/<id>` → same `.ep-item` shape.
- `sources`: `/api/theme/episode/servers?episodeId=` → `data-type` +
  `data-server-name` + base64 `data-hash`, which **decodes straight to the
  embed URL** (one fewer request than HiAnimeX). ZokoAnime → direct HLS +
  subtitles; others (HD-1/MegaPlay…) → `EmbedPage` fallback. Direct-first
  sort, same as HiAnimeX.

## `watchanimeworld.rs` — WatchAnimeWorld

`https://watchanimeworld.one`, id `watchanimeworld`. WordPress site:

- `search`: `/?s=<q>`; `episodes`: `/episode/<slug>-<s>x<e>/` pages;
  `sources`: inline `var CONFIG` JSON with per-language Abyss tokens
  (`https://player.abyssplayer.com/` + token). **All sources are
  `EmbedPage`** — browser-only, no direct extraction.

## `animostream.rs` — AnimoStream

`https://animostream.xyz`, id `animostream`. Blogger site:
`/search?q=` listings, episodes as in-page JS buttons swapping iframes
(abyssplayer.com / animostream.embedseek.online), language tags from
listing excerpts. **Cloudflare WAF 403s cold automated requests** —
returns a "press Tab" error in practice. All `EmbedPage`.

## Tests

- `mod.rs`: `parse_languages` (dual-audio title, Japanese dedupe, empty).
- `hianimeat.rs`: `WATCH_ID` parsing, `SERVER_ATTRS` regex,
  `data-hash` base64 decode.
- `tests/live_smoke.rs` (ignored, need network): hianimex
  search+episodes, hianimex server chain (asserts sub+dub Direct first),
  zokoanime extractor, watchanimeworld search, and
  `hianimeat_full_chain_with_subtitles` — the end-to-end proof: search →
  episodes → sources, first source is in-player kind, subtitle present,
  and the m3u8 serves a valid `#EXTM3U` playlist.

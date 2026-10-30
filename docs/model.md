# model.rs — data model

Plain structs shared by every provider, the app state, and the UI.

## `Anime`
One series/season from a search/trending/history list.
`id`, `title`, `url` (canonical provider page), `poster: Option<String>`,
`languages: Vec<String>` (shown **before** watching — the user's core
ask), `note: String` (e.g. `"EP 3"` on history rows, `"S01 EP 1-16 Added"`),
`provider_id: String`.
`display_title()` → `"Title  [Hindi, English]"` (or bare title).

## `AnimeDetails`
`anime: Anime` + `synopsis` (900 chars, whitespace-collapsed) +
`genres: Vec<String>` (max 10) + `status` + `year`. Shown on the Detail
screen with the Languages line above the episode list.

## `Episode`
`number: String`, `title: String`, `url: String`,
`languages: Vec<String>` (per-episode; may differ), `servers: Vec<ServerRef>`.

## `ServerRef`
A named playback endpoint: `name` (e.g. `"ep"`, `"Hindi"`, `"SUB · HD-2"`),
`url` (often a raw id the provider's `sources()` understands), and
`language: Option<String>`.

## `StreamSource`
One playable stream:
`label` (e.g. `"Japanese (Sub) · Direct"`), `url` (direct media URL or
embed page), `quality: Option<String>`, `kind: SourceKind`,
`headers: Vec<(String, String)>` (Referer etc.), and
`subtitle_url: Option<String>` (v0.1.7 — fed to the player's sub-file arg).
`referer()` — case-insensitive lookup of the Referer header.

## `SourceKind`
`Direct` (.mp4/.mkv), `Hls` (.m3u8 — mpv/vlc handle natively),
`EmbedPage` (watch page → opened in the browser as a fallback).

## `HistoryEntry`
One played episode, serialized to `history.json`:
`anime_id`, `anime_title`, `anime_url`, `provider_id`,
`episode_number`, `episode_title`, `played_at_unix`.
`as_anime()` → the home-screen row (title + `"EP n"` note).

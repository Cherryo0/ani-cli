# extractors.rs — embed → direct stream

Turns embed *pages* (HTML) into directly playable URLs, so the TUI can
hand them to mpv instead of opening a browser tab.

## `ResolvedStream`
- `url` — direct media URL (HLS master playlist).
- `referer` — the embed page URL; the player must send it (the stream host
  checks it).
- `subtitle_url: Option<String>` — default subtitle track (v0.1.7).

## `resolve_zokoanime(embed_url) -> Result<ResolvedStream>` (async)

Cracks ZokoAnime's player config. The embed page ships
`window.__P = base64(JSON XOR "otaku-embed-v1")` (their
`/core/obfuscate.js`); the decoded JSON's `src` is a direct HLS URL on
`hls.dramahot.top`.

Steps: GET embed page (Referer: provider) → regex `window.__P="…"` →
base64-decode → XOR each byte with cycling key `otaku-embed-v1` →
UTF-8 → JSON → `src`. Then `parse_subtitle_url` for subs.

Verified live: the `src` m3u8 serves a valid 1080p variant playlist
(360p/720p/1080p variants).

## `parse_subtitle_url(v: &Value, embed_url: &str) -> Option<String>` (private)

From the deobfuscated JSON's `subtitles` array: prefer the track with
`"default": true`, fall back to the first track. Relative `src`s are
resolved against the embed origin. (Mirrors pystardust/ani-cli's
`--sub-file` behavior.)

## `base64_decode(s) -> Result<Vec<u8>>` (private)

Hand-rolled base64 (no extra dep), tolerates missing padding.

## Tests

- `xor_roundtrip` — obfuscate→deobfuscate round-trip with the real key.
- `base64_decodes` — padded and unpadded.
- `subtitle_prefers_default_track`, `subtitle_falls_back_to_first_and_resolves_relative`,
  `subtitle_none_when_absent`.

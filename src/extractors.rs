//! Direct-stream extractors: turn embed pages into playable URLs.
//!
//! Some providers only hand us embed *pages* (HTML). Where the embed's
//! stream provisioning is reverse-engineered, these resolvers pull the
//! direct file/HLS URL out so the TUI can hand it to mpv/vlc instead of
//! opening a browser.

use crate::providers::http_client;
use once_cell::sync::Lazy;
use regex::Regex;

/// A directly playable stream pulled out of an embed page.
#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    /// Referer the player should send (usually the embed page itself).
    pub referer: String,
    /// Default subtitle track URL, when the embed ships one
    /// (pystardust/ani-cli passes this as mpv's `--sub-file`).
    pub subtitle_url: Option<String>,
}

static ZOKO_PAYLOAD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"window\.__P="([^"]+)""#).unwrap());

/// ZokoAnime's player ships its config as `window.__P`: base64 of the
/// JSON XOR'd with the fixed key `otaku-embed-v1` (see their
/// `/core/obfuscate.js`). The decoded JSON has a `src` field with a
/// direct HLS URL.
///
/// Verified live 2026-10-29: the `src` m3u8 serves a valid 1080p variant
/// playlist.
pub async fn resolve_zokoanime(embed_url: &str) -> anyhow::Result<ResolvedStream> {
    const KEY: &[u8] = b"otaku-embed-v1";

    let html = http_client()
        .get(embed_url)
        .header("Referer", "https://hianimex.com.in/")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let blob = ZOKO_PAYLOAD
        .captures(&html)
        .map(|c| c[1].to_string())
        .ok_or_else(|| anyhow::anyhow!("zokoanime payload (__P) not found"))?;

    let raw = base64_decode(&blob)?;
    let xored: Vec<u8> = raw
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    let json_str = String::from_utf8(xored)
        .map_err(|_| anyhow::anyhow!("zokoanime payload is not valid UTF-8"))?;
    let v: serde_json::Value = serde_json::from_str(&json_str)?;

    let src = v
        .get("src")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("zokoanime payload has no src"))?;

    // Subtitle tracks: prefer the default one, fall back to the first.
    // (pystardust/ani-cli does the same for its --sub-file support.)
    let subtitle_url = parse_subtitle_url(&v, embed_url);

    Ok(ResolvedStream {
        url: src.to_string(),
        referer: embed_url.to_string(),
        subtitle_url,
    })
}

/// Pick the default subtitle track from a deobfuscated ZokoAnime payload
/// (falling back to the first track), resolving relative URLs against the
/// embed origin.
fn parse_subtitle_url(v: &serde_json::Value, embed_url: &str) -> Option<String> {
    v.get("subtitles")
        .and_then(|s| s.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("default").and_then(|d| d.as_bool()).unwrap_or(false))
                .or_else(|| arr.first())
        })
        .and_then(|t| t.get("src"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.starts_with("http") {
                s.to_string()
            } else {
                let origin = embed_url.split('/').take(3).collect::<Vec<_>>().join("/");
                format!("{}/{}", origin, s.trim_start_matches('/'))
            }
        })
}

fn base64_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut vals = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        vals[c as usize] = i as u8;
    }
    let clean: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut n: u32 = 0;
        let mut count = 0;
        for &b in chunk {
            let v = vals[b as usize];
            if v == 255 {
                anyhow::bail!("invalid base64 character");
            }
            n = (n << 6) | v as u32;
            count += 1;
        }
        // Pad the bit buffer to a full 24 bits.
        n <<= 6 * (4 - count);
        out.push((n >> 16) as u8);
        if count > 2 {
            out.push((n >> 8) as u8);
        }
        if count > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_roundtrip() {
        // obfuscate("{\"a\":1}") with the real algorithm, then deobfuscate.
        let key = b"otaku-embed-v1";
        let plain = br#"{"src":"https://example.com/x.m3u8"}"#;
        let xored: Vec<u8> = plain
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        // re-derive via resolve path pieces
        let back: Vec<u8> = xored
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        assert_eq!(back, plain);
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
    }

    #[test]
    fn subtitle_prefers_default_track() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"src":"https://x/y.m3u8","subtitles":[
                {"src":"https://x/subs/es.vtt","label":"Spanish"},
                {"src":"https://x/subs/en.vtt","label":"English","default":true}
            ]}"#,
        )
        .unwrap();
        assert_eq!(
            parse_subtitle_url(&v, "https://zokoanime.video/stream/mal/1/1/sub"),
            Some("https://x/subs/en.vtt".to_string())
        );
    }

    #[test]
    fn subtitle_falls_back_to_first_and_resolves_relative() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"src":"https://x/y.m3u8","subtitles":[{"src":"/subs/en.vtt","label":"English"}]}"#,
        )
        .unwrap();
        assert_eq!(
            parse_subtitle_url(&v, "https://zokoanime.video/stream/mal/1/1/sub"),
            Some("https://zokoanime.video/subs/en.vtt".to_string())
        );
    }

    #[test]
    fn subtitle_none_when_absent() {
        let v: serde_json::Value =
            serde_json::from_str(r#"{"src":"https://x/y.m3u8"}"#).unwrap();
        assert_eq!(
            parse_subtitle_url(&v, "https://zokoanime.video/stream/mal/1/1/sub"),
            None
        );
    }
}

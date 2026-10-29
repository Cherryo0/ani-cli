//! HiAnimeAt provider — https://hianime.at/
//!
//! This is the backend pystardust/ani-cli (v5.x) uses, added at the user's
//! request. Same HiAnime network as HiAnimeX, but a different API generation:
//! * Search: `/search?keyword=<q>` → `.flw-item` cards (same markup).
//! * Anime id: trailing `-<id>` of the `/watch/<slug>-<id>` URL.
//! * Episodes: `/api/theme/episode/list/<animeId>` → JSON `{"html": "..."}`
//!   with `.ep-item` anchors carrying `data-number` + `data-id`.
//! * Servers: `/api/theme/episode/servers?episodeId=<epId>` → items with
//!   `data-type` (sub/dub), `data-server-name` and a base64 `data-hash`
//!   that decodes straight to the embed URL (no extra sources call needed).
//!
//! The `ZokoAnime` server's embed is deobfuscated (same `otaku-embed-v1`
//! XOR scheme) into a direct HLS URL with subtitles, so playback lands in
//! mpv. Other servers are returned as `EmbedPage` browser fallbacks.

use super::{http_client, Provider};
use crate::extractors;
use crate::model::{Anime, AnimeDetails, Episode, ServerRef, SourceKind, StreamSource};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

pub struct HiAnimeAt;

const BASE: &str = "https://hianime.at";
const PROVIDER_ID: &str = "hianimeat";

static WATCH_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"/watch/.+-(\d+)/?$").unwrap());
static SERVER_ATTRS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-type="([a-z]+)"[^>]*data-server-name="([^"]+)"[^>]*data-hash="([^"]+)""#)
        .unwrap()
});
static EP_ATTRS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-number="(\d+)"[^>]*data-id="(\d+)""#).unwrap()
});

async fn get(url: &str) -> anyhow::Result<String> {
    let text = http_client()
        .get(url)
        .header("Referer", format!("{BASE}/"))
        .header("X-Requested-With", "XMLHttpRequest")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        )
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(text)
}

fn anime_id_from_url(url: &str) -> Option<String> {
    WATCH_ID.captures(url).map(|c| c[1].to_string())
}

fn b64_decode(s: &str) -> anyhow::Result<String> {
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
        n <<= 6 * (4 - count);
        out.push((n >> 16) as u8);
        if count > 2 {
            out.push((n >> 8) as u8);
        }
        if count > 3 {
            out.push(n as u8);
        }
    }
    String::from_utf8(out).map_err(|_| anyhow::anyhow!("hash is not valid UTF-8"))
}

fn parse_cards(html: &str) -> Vec<Anime> {
    let doc = Html::parse_document(html);
    let mut out: Vec<Anime> = Vec::new();
    let Ok(item_sel) = Selector::parse(".flw-item") else { return out };
    for item in doc.select(&item_sel) {
        let link_sel = Selector::parse("a.film-poster-ahref, a[href^=\"/watch/\"]").unwrap();
        let name_sel = Selector::parse(".film-name").unwrap();
        let (a, name) = match (item.select(&link_sel).next(), item.select(&name_sel).next()) {
            (Some(a), Some(n)) => (
                a,
                n.text().collect::<Vec<_>>().join(" ").trim().to_string(),
            ),
            _ => continue,
        };
        let href = a.value().attr("href").unwrap_or("");
        let data_id = a.value().attr("data-id").map(str::to_string);
        if href.is_empty() || name.is_empty() {
            continue;
        }
        let url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("{BASE}{href}")
        };
        let poster = Selector::parse("img")
            .ok()
            .and_then(|s| item.select(&s).next())
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
                    .map(str::to_string)
            });
        let has_sub = Selector::parse(".tick-sub")
            .ok()
            .map(|s| item.select(&s).next().is_some())
            .unwrap_or(false);
        let has_dub = Selector::parse(".tick-dub")
            .ok()
            .map(|s| item.select(&s).next().is_some())
            .unwrap_or(false);
        let mut languages: Vec<String> = Vec::new();
        if has_sub {
            languages.push("Japanese".to_string());
        }
        if has_dub {
            languages.push("English".to_string());
        }
        out.push(Anime {
            id: data_id
                .or_else(|| anime_id_from_url(&url))
                .unwrap_or_else(|| url.clone()),
            title: name,
            url,
            poster,
            languages,
            note: String::new(),
            provider_id: PROVIDER_ID.to_string(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    out.into_iter()
        .filter(|a| seen.insert(a.url.clone()))
        .collect()
}

#[async_trait]
impl Provider for HiAnimeAt {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }
    fn name(&self) -> &'static str {
        "HiAnimeAt"
    }
    fn base_url(&self) -> &'static str {
        BASE
    }

    async fn trending(&self) -> anyhow::Result<Vec<Anime>> {
        let mut cards = parse_cards(&get(&format!("{BASE}/home")).await?);
        cards.truncate(24);
        Ok(cards)
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Anime>> {
        let url = format!("{BASE}/search?keyword={}", query.replace(' ', "+"));
        // The sidebar repeats result markup — cut it off first.
        let html = get(&url).await?;
        let html = html.split("id=\"main-sidebar\"").next().unwrap_or(&html);
        Ok(parse_cards(html))
    }

    async fn details(&self, anime: &Anime) -> anyhow::Result<AnimeDetails> {
        let html = get(&anime.url).await?;
        let doc = Html::parse_document(&html);
        let mut synopsis = String::new();
        if let Ok(s) = Selector::parse(".film-description, .description") {
            if let Some(d) = doc.select(&s).next() {
                synopsis = d.text().collect::<Vec<_>>().join(" ").trim().to_string();
                synopsis = synopsis.split_whitespace().collect::<Vec<_>>().join(" ");
                synopsis.truncate(900);
            }
        }
        let mut genres: Vec<String> = Vec::new();
        if let Ok(gs) = Selector::parse(".anisc-info a[href*='/genre/']") {
            for a in doc.select(&gs).take(10) {
                let g: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !g.is_empty() && !genres.contains(&g) {
                    genres.push(g);
                }
            }
        }
        Ok(AnimeDetails {
            anime: anime.clone(),
            synopsis,
            genres,
            status: String::new(),
            year: String::new(),
        })
    }

    async fn episodes(&self, anime: &Anime) -> anyhow::Result<Vec<Episode>> {
        let anime_id = if anime.id.chars().all(|c| c.is_ascii_digit()) {
            anime.id.clone()
        } else {
            anime_id_from_url(&anime.url)
                .ok_or_else(|| anyhow::anyhow!("could not parse anime id from {}", anime.url))?
        };
        let url = format!("{BASE}/api/theme/episode/list/{anime_id}");
        let body = get(&url).await?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let html = v.get("html").and_then(|h| h.as_str()).unwrap_or("");
        let mut episodes: Vec<Episode> = Vec::new();
        for cap in EP_ATTRS.captures_iter(html) {
            let number: u32 = cap[1].parse().unwrap_or(0);
            let ep_id = cap[2].to_string();
            if number == 0 {
                continue;
            }
            episodes.push(Episode {
                number: number.to_string(),
                title: format!("Episode {number}"),
                url: format!("{BASE}/watch/{}?ep={ep_id}", anime.id),
                languages: anime.languages.clone(),
                servers: vec![ServerRef {
                    name: "ep".to_string(),
                    url: ep_id,
                    language: None,
                }],
            });
        }
        episodes.sort_by_key(|e| e.number.parse::<u32>().unwrap_or(0));
        if episodes.is_empty() {
            anyhow::bail!("No episodes found for this anime");
        }
        Ok(episodes)
    }

    async fn sources(&self, episode: &Episode) -> anyhow::Result<Vec<StreamSource>> {
        let ep_id = episode
            .servers
            .first()
            .map(|s| s.url.clone())
            .ok_or_else(|| anyhow::anyhow!("episode id missing"))?;
        let url = format!("{BASE}/api/theme/episode/servers?episodeId={ep_id}");
        let body = get(&url).await?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let html = v.get("html").and_then(|h| h.as_str()).unwrap_or("");

        let mut out: Vec<StreamSource> = Vec::new();
        for cap in SERVER_ATTRS.captures_iter(html) {
            let dtype = &cap[1]; // sub | dub
            let server_name = &cap[2];
            let hash = &cap[3];
            let lang = match dtype {
                "dub" => "English (Dub)",
                _ => "Japanese (Sub)",
            };
            // The hash decodes straight to the embed URL.
            let embed_url = match b64_decode(hash) {
                Ok(u) => u,
                Err(_) => continue,
            };
            // ZokoAnime embeds carry a directly playable HLS URL (+subs).
            if embed_url.contains("zokoanime.video") {
                match extractors::resolve_zokoanime(&embed_url).await {
                    Ok(direct) => {
                        out.push(StreamSource {
                            label: format!("{lang} · Direct"),
                            url: direct.url,
                            quality: Some("HD".to_string()),
                            kind: SourceKind::Hls,
                            headers: vec![("Referer".to_string(), direct.referer)],
                            subtitle_url: direct.subtitle_url,
                        });
                        continue;
                    }
                    Err(_) => {} // fall through to the embed-page fallback
                }
            }
            out.push(StreamSource {
                label: format!("{lang} · {server_name}"),
                url: embed_url,
                quality: None,
                kind: SourceKind::EmbedPage,
                headers: vec![("Referer".to_string(), format!("{BASE}/"))],
                subtitle_url: None,
            });
        }
        if out.is_empty() {
            anyhow::bail!("No playable sources found for this episode");
        }
        // Direct sources first so the default pick plays in mpv.
        out.sort_by_key(|s| match s.kind {
            SourceKind::Hls | SourceKind::Direct => 0,
            SourceKind::EmbedPage => 1,
        });
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_id_parses() {
        assert_eq!(
            anime_id_from_url("https://hianime.at/watch/naruto-1335"),
            Some("1335".to_string())
        );
    }

    #[test]
    fn server_attrs_parse() {
        let html = r#"<div class="server-item" data-type="sub" data-server-name="ZokoAnime" data-hash="aHR0cHM6Ly8=">"#;
        let cap = SERVER_ATTRS.captures(html).unwrap();
        assert_eq!(&cap[1], "sub");
        assert_eq!(&cap[2], "ZokoAnime");
        assert_eq!(&cap[3], "aHR0cHM6Ly8=");
    }

    #[test]
    fn hash_decodes_to_embed_url() {
        assert_eq!(
            b64_decode("aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC82MTE4NC8xL3N1Yg==")
                .unwrap(),
            "https://zokoanime.video/stream/mal/61184/1/sub"
        );
    }
}

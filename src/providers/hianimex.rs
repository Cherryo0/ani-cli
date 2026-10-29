//! HiAnimeX provider — https://hianimex.com.in/
//!
//! Reverse-engineered 2026-10-29 (read-only). The site is a HiAnime clone
//! exposing the classic v2 ajax API (verified live):
//! * Search: `/search?keyword=<q>` → `.flw-item` cards with `/watch/<slug>-<id>`
//!   links, poster, name, and SUB/DUB tick badges (used for language tags).
//! * Anime id: trailing `-<id>` segment of the watch URL.
//! * Episodes: `/ajax/v2/episode/list/<animeId>` → JSON `{"html": "..."}`
//!   with `.ep-item` anchors carrying `data-id` (episode id) + `data-number`.
//! * Servers: `/ajax/v2/episode/servers?episodeId=<epId>` → `data-type`
//!   (`sub`/`dub`) + `data-id` (server id) items.
//! * Sources: `/ajax/v2/episode/sources?id=<serverId>` → JSON
//!   `{"type":"iframe","link":"https://megaplay.buzz/…",…}`.
//!
//! Server embeds (megaplay/megavid/zokoanime) are returned as `EmbedPage`
//! sources — the app opens them in the user's browser. The SUB/DUB split is
//! surfaced per source so the audio choice is visible up front.

use super::{http_client, Provider};
use crate::extractors;
use crate::model::{Anime, AnimeDetails, Episode, ServerRef, SourceKind, StreamSource};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

pub struct HiAnimeX;

const BASE: &str = "https://hianimex.com.in";
const PROVIDER_ID: &str = "hianimex";

static WATCH_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"/watch/.+-(\d+)/?$").unwrap());
static SERVER_ATTRS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-type="([a-z]+)"[^>]*data-id="(\d+)"[^>]*data-server-id="(\d+)""#).unwrap()
});
static EP_ATTRS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-number="(\d+)"[^>]*data-id="(\d+)""#).unwrap()
});

async fn get(url: &str) -> anyhow::Result<String> {
    let text = http_client()
        .get(url)
        .header("Referer", format!("{BASE}/"))
        .header("X-Requested-With", "XMLHttpRequest")
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
        // The poster anchor carries the numeric anime id — most reliable.
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
        // SUB/DUB badges → language tags shown before watching.
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
            languages.push("Japanese".to_string()); // subbed
        }
        if has_dub {
            languages.push("English".to_string()); // dubbed
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
impl Provider for HiAnimeX {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }
    fn name(&self) -> &'static str {
        "HiAnimeX"
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
        Ok(parse_cards(&get(&url).await?))
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
        let url = format!("{BASE}/ajax/v2/episode/list/{anime_id}");
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
            // Title from the anchor's title attribute when present.
            let title_re = Regex::new(&format!(
                r#"data-id="{ep_id}"[^>]*title="([^"]+)""#
            ))
            .unwrap();
            let title = title_re
                .captures(html)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| format!("Episode {number}"));
            let slug = anime.url.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");
            episodes.push(Episode {
                number: number.to_string(),
                title: title.chars().take(80).collect(),
                url: format!("{BASE}/watch/{slug}?ep={ep_id}"),
                languages: anime.languages.clone(),
                // sources() reads the raw episode id from here.
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
        let url = format!("{BASE}/ajax/v2/episode/servers?episodeId={ep_id}");
        let body = get(&url).await?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let html = v.get("html").and_then(|h| h.as_str()).unwrap_or("");

        let mut out: Vec<StreamSource> = Vec::new();
        let mut n = 0;
        for cap in SERVER_ATTRS.captures_iter(html) {
            let dtype = &cap[1]; // sub | dub
            let server_id = &cap[2];
            n += 1;
            let src_url = format!("{BASE}/ajax/v2/episode/sources?id={server_id}");
            let src_body = get(&src_url).await?;
            let src_v: serde_json::Value = match serde_json::from_str(&src_body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let link = src_v.get("link").and_then(|l| l.as_str()).unwrap_or("");
            if link.is_empty() {
                continue;
            }
            let lang = match dtype {
                "dub" => "English (Dub)",
                _ => "Japanese (Sub)",
            };
            // ZokoAnime embeds carry a directly playable HLS URL — extract
            // it so the stream opens in mpv instead of a browser tab.
            if link.contains("zokoanime.video") {
                match extractors::resolve_zokoanime(link).await {
                    Ok(direct) => {
                        out.push(StreamSource {
                            label: format!("{lang} · Direct"),
                            url: direct.url,
                            quality: Some("HD".to_string()),
                            kind: SourceKind::Hls,
                            headers: vec![("Referer".to_string(), direct.referer)],
                        });
                        continue;
                    }
                    Err(_) => {} // fall through to the embed-page fallback
                }
            }
            out.push(StreamSource {
                label: format!("{lang} · Server {n}"),
                url: link.to_string(),
                quality: None,
                kind: SourceKind::EmbedPage,
                headers: vec![("Referer".to_string(), format!("{BASE}/"))],
            });
        }
        if out.is_empty() {
            anyhow::bail!("No playable sources found for this episode");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_id_parses() {
        assert_eq!(
            anime_id_from_url("https://hianimex.com.in/watch/naruto-shippuden-the-movie-vv6fp-43"),
            Some("43".to_string())
        );
    }

    #[test]
    fn server_attrs_parse() {
        let html = r#"<div class="server-item" data-type="sub" data-id="7600" data-server-id="34801">"#;
        let cap = SERVER_ATTRS.captures(html).unwrap();
        assert_eq!(&cap[1], "sub");
        assert_eq!(&cap[2], "7600");
    }

    #[test]
    fn episode_attrs_parse() {
        let html = r#"data-number="1" data-id="76" href="/watch/x-43?ep=76""#;
        let cap = EP_ATTRS.captures(html).unwrap();
        assert_eq!(&cap[1], "1");
        assert_eq!(&cap[2], "76");
    }
}

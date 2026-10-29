//! AnimoStream provider — https://www.animostream.xyz/
//!
//! Reverse-engineered 2026-10-29 (read-only):
//! * The site runs on **Blogger**, not WordPress. Search is Blogger-native:
//!   `/search?q=<query>` renders article cards (poster, title link, excerpt
//!   carrying language/quality tags like
//!   "Dual/Multi Audio [Hindi ORG, ENG ] WEB-DL 720p HEVC").
//! * Anime pages live at `/YYYY/MM/<slug>.html`. Episodes are NOT separate
//!   pages: one in-page JS player swaps an `<iframe>` per episode/server.
//!   Server 1 embeds look like `https://abyssplayer.com/<TOKEN>?t=...`,
//!   Server 2 like `https://animostream.embedseek.online/#<token>` — one
//!   opaque token per server/season/episode, embedded in the page's JS.
//! * There is no public JSON API; `/wp-json/` and `/feeds` are WAF-blocked.
//!
//! **Cloudflare caveat:** the site's WAF blocks cold automated requests
//! (HTTP 403). Requests carry a full browser header set plus a same-site
//! `Referer`; when the WAF still blocks, the provider returns a clear error
//! telling the user to switch providers with `Tab`.

use super::{http_client, parse_languages, Provider};
use crate::model::{Anime, AnimeDetails, Episode, ServerRef, SourceKind, StreamSource};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

pub struct AnimoStream;

const BASE: &str = "https://www.animostream.xyz";
const PROVIDER_ID: &str = "animostream";

static TOKEN_ABYSS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"abyssplayer\.com/([A-Za-z0-9_-]{4,})").unwrap());
static TOKEN_EMBEDSEEK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"embedseek\.online/#([a-z0-9]+)").unwrap());
static SEASON_OPT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<option[^>]*>\s*Season\s+(\d+)").unwrap());

fn waf_err() -> anyhow::Error {
    anyhow::anyhow!(
        "AnimoStream blocked the request (Cloudflare 403). \
         Press Tab to use WatchAnimeWorld or HiAnimeX instead."
    )
}

async fn get(url: &str) -> anyhow::Result<String> {
    let resp = http_client()
        .get(url)
        .header("Referer", format!("{BASE}/"))
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(waf_err());
    }
    let resp = resp.error_for_status()?;
    Ok(resp.text().await?)
}

/// Parse Blogger article cards into [`Anime`] entries. Tries several
/// selector shapes because Blogger templates vary.
fn parse_cards(html: &str) -> Vec<Anime> {
    let doc = Html::parse_document(html);
    let mut out: Vec<Anime> = Vec::new();

    for card_sel in ["article", ".post-card", ".blog-posts .post", "div.post"] {
        let Ok(sel) = Selector::parse(card_sel) else { continue };
        let cards: Vec<_> = doc.select(&sel).collect();
        if cards.is_empty() {
            continue;
        }
        for card in cards {
            let mut title = String::new();
            let mut url = String::new();
            for link_sel in ["h2 a", "h3 a", ".post-title a", "a"] {
                let Ok(ls) = Selector::parse(link_sel) else { continue };
                if let Some(a) = card.select(&ls).next() {
                    let t: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !t.is_empty() {
                        title = t;
                        url = a.value().attr("href").unwrap_or("").to_string();
                        break;
                    }
                }
            }
            if title.is_empty() || url.is_empty() {
                continue;
            }
            if !url.starts_with("http") {
                url = format!("{BASE}/{}", url.trim_start_matches('/'));
            }
            let poster = Selector::parse("img")
                .ok()
                .and_then(|s| card.select(&s).next())
                .and_then(|img| img.value().attr("src").map(str::to_string));
            let mut excerpt = String::new();
            for ex_sel in [".post-excerpt", ".snippet", ".post-snippet", "p"] {
                let Ok(es) = Selector::parse(ex_sel) else { continue };
                if let Some(e) = card.select(&es).next() {
                    let t: String = e.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if t.len() > excerpt.len() {
                        excerpt = t;
                    }
                }
            }
            let languages = parse_languages(&format!("{title} {excerpt}"));
            let note: String = excerpt
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .chars()
                .take(90)
                .collect();
            out.push(Anime {
                id: url.clone(),
                title,
                url,
                poster,
                languages,
                note,
                provider_id: PROVIDER_ID.to_string(),
            });
        }
        if !out.is_empty() {
            break;
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.into_iter()
        .filter(|a| seen.insert(a.url.clone()))
        .collect()
}

/// Extract per-server episode tokens from the post page's inline JS.
/// Returns (server_name, tokens_in_episode_order).
fn extract_servers(html: &str) -> Vec<(String, Vec<String>)> {
    let mut servers: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let abyss: Vec<String> = TOKEN_ABYSS
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .filter(|t| seen.insert(t.clone()))
        .collect();
    if !abyss.is_empty() {
        servers.push(("Server 1 · Abyss".to_string(), abyss));
    }
    let mut seen2 = std::collections::HashSet::new();
    let embedseek: Vec<String> = TOKEN_EMBEDSEEK
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .filter(|t| seen2.insert(t.clone()))
        .collect();
    if !embedseek.is_empty() {
        servers.push(("Server 2 · EmbedSeek".to_string(), embedseek));
    }
    servers
}

fn seasons_in(html: &str) -> Vec<u32> {
    let mut seasons: Vec<u32> = SEASON_OPT
        .captures_iter(html)
        .filter_map(|c| c[1].parse().ok())
        .collect();
    seasons.sort_unstable();
    seasons.dedup();
    if seasons.is_empty() {
        seasons.push(1);
    }
    seasons
}

#[async_trait]
impl Provider for AnimoStream {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }
    fn name(&self) -> &'static str {
        "AnimoStream"
    }
    fn base_url(&self) -> &'static str {
        BASE
    }

    async fn trending(&self) -> anyhow::Result<Vec<Anime>> {
        let html = get(BASE).await?;
        Ok(parse_cards(&html))
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Anime>> {
        let url = format!("{BASE}/search?q={}", urlencode(query));
        let html = get(&url).await?;
        Ok(parse_cards(&html))
    }

    async fn details(&self, anime: &Anime) -> anyhow::Result<AnimeDetails> {
        let html = get(&anime.url).await?;
        let doc = Html::parse_document(&html);
        let h1 = Selector::parse("h1").unwrap();
        let title = doc
            .select(&h1)
            .next()
            .map(|e| e.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| anime.title.clone());
        let mut genres: Vec<String> = Vec::new();
        if let Ok(ls) = Selector::parse(".post-labels a, .labels a") {
            for a in doc.select(&ls).take(8) {
                let g: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !g.is_empty() && !genres.contains(&g) {
                    genres.push(g);
                }
            }
        }
        let mut synopsis = String::new();
        if let Ok(bs) = Selector::parse(".post-body") {
            if let Some(b) = doc.select(&bs).next() {
                synopsis = b.text().collect::<Vec<_>>().join(" ");
                synopsis = synopsis.split_whitespace().collect::<Vec<_>>().join(" ");
                synopsis.truncate(900);
            }
        }
        Ok(AnimeDetails {
            anime: Anime {
                title,
                ..anime.clone()
            },
            synopsis,
            genres,
            status: String::new(),
            year: String::new(),
        })
    }

    async fn episodes(&self, anime: &Anime) -> anyhow::Result<Vec<Episode>> {
        let html = get(&anime.url).await?;
        let servers = extract_servers(&html);
        let seasons = seasons_in(&html);

        let per_season = servers.iter().map(|(_, t)| t.len()).max().unwrap_or(0);
        if per_season == 0 {
            anyhow::bail!(
                "No episode player data found on this page (the site may have changed its layout)"
            );
        }
        let mut episodes: Vec<Episode> = Vec::new();
        for &season in &seasons {
            for n in 1..=per_season {
                let mut refs: Vec<ServerRef> = Vec::new();
                for (sname, tokens) in &servers {
                    if let Some(tok) = tokens.get(n - 1) {
                        let url = if sname.contains("Abyss") {
                            format!("https://abyssplayer.com/{tok}")
                        } else {
                            format!("https://animostream.embedseek.online/#{tok}")
                        };
                        refs.push(ServerRef {
                            name: sname.clone(),
                            url,
                            language: None,
                        });
                    }
                }
                episodes.push(Episode {
                    number: format!("S{season}E{n}"),
                    title: anime.title.clone(),
                    url: anime.url.clone(),
                    languages: anime.languages.clone(),
                    servers: refs,
                });
            }
        }
        Ok(episodes)
    }

    async fn sources(&self, episode: &Episode) -> anyhow::Result<Vec<StreamSource>> {
        Ok(episode
            .servers
            .iter()
            .map(|s| StreamSource {
                label: s.name.clone(),
                url: s.url.clone(),
                quality: None,
                kind: SourceKind::EmbedPage,
                headers: vec![("Referer".to_string(), format!("{BASE}/"))],
                subtitle_url: None,
            })
            .collect())
    }
}

/// Minimal percent-encoding for query strings.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_abyss_and_embedseek_tokens() {
        let html = r#"<script>var a="https://abyssplayer.com/XbXS8st1z?t=1";var b="https://animostream.embedseek.online/#cnekk";var c="https://animostream.embedseek.online/#elkkl";</script>"#;
        let servers = extract_servers(html);
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].1, vec!["XbXS8st1z"]);
        assert_eq!(servers[1].1, vec!["cnekk", "elkkl"]);
    }

    #[test]
    fn finds_seasons() {
        let html = r#"<select><option>Season 1</option><option>Season 2</option></select>"#;
        assert_eq!(seasons_in(html), vec![1, 2]);
    }

    #[test]
    fn query_is_encoded() {
        assert_eq!(urlencode("kaiju no 8"), "kaiju+no+8");
    }
}

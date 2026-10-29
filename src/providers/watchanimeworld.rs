//! WatchAnimeWorld provider — https://watchanimeworld.one/
//!
//! Reverse-engineered 2026-10-29 (read-only):
//! * WordPress site with custom post types `movies` / `series`. Cards are
//!   `<article class="post-… type-series … category-hindi category-english
//!   category-japanese …">` — the `category-*` classes double as **language
//!   tags**, parsed into the language badges shown before you pick anything.
//! * Search is plain WordPress: `/?s=<query>`.
//! * Series pages list episodes as links like
//!   `/episode/<slug>-<season>x<episode>/`.
//! * Episode pages embed two players: a `play.zephyrix.org` iframe and a
//!   `/dub-player/t/<id>/` page whose inline `CONFIG` JSON maps language
//!   codes to per-language Abyss tokens, e.g.
//!   `"ready":{"hin":"4VL4zDfX7","eng":"SOtgCy3hI","jpn":"KPwvpKLL6"}` with
//!   `"prefix":"https://player.abyssplayer.com/"` and a `lang` name table.
//!   Each language becomes its own labelled source — the user sees the
//!   audio options *before* pressing play.

use super::{http_client, normalise_language, parse_languages, Provider};
use crate::model::{Anime, AnimeDetails, Episode, SourceKind, StreamSource};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;

pub struct WatchAnimeWorld;

const BASE: &str = "https://watchanimeworld.one";
const PROVIDER_ID: &str = "watchanimeworld";

static EP_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/episode/.+-(\d+)x(\d+)/?$").unwrap());
static CAT_LANG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"category-([a-z]+)").unwrap());

/// WordPress category slugs that denote a language.
fn category_language(slug: &str) -> Option<String> {
    match slug {
        "hindi" | "english" | "japanese" | "tamil" | "telugu" | "bengali" | "malayalam"
        | "kannada" | "punjabi" | "urdu" | "marathi" | "korean" | "chinese" | "french"
        | "spanish" | "german" | "portuguese" | "italian" | "russian" | "arabic" => {
            Some(normalise_language(slug))
        }
        _ => None,
    }
}

async fn get(url: &str) -> anyhow::Result<String> {
    let text = http_client()
        .get(url)
        .header("Referer", format!("{BASE}/"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(text)
}

fn parse_cards(html: &str) -> Vec<Anime> {
    let doc = Html::parse_document(html);
    let mut out: Vec<Anime> = Vec::new();
    // Cards come in two shapes: homepage `<article class="… type-series …">`
    // and search-result `<li class="… type-series …"><article>…` where the
    // language `category-*` classes live on the outer `<li>`.
    let Ok(card_sel) = Selector::parse("li[class*='type-'], article[class*='type-']") else {
        return out;
    };
    for card in doc.select(&card_sel) {
        let class = card.value().attr("class").unwrap_or("");
        if !(class.contains("type-movies") || class.contains("type-series")) {
            continue;
        }
        // Title: may be a bare heading (search) or a heading link (home).
        let mut title = String::new();
        for ts in [".entry-title", "h2", "h3", ".post-title"] {
            let Ok(ls) = Selector::parse(ts) else { continue };
            if let Some(e) = card.select(&ls).next() {
                title = e.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !title.is_empty() {
                    break;
                }
            }
        }
        // URL: overlay link `a.lnk-blk`, else the title link.
        let mut url = String::new();
        for us in ["a.lnk-blk", ".entry-title a", "h2 a"] {
            let Ok(ls) = Selector::parse(us) else { continue };
            if let Some(a) = card.select(&ls).next() {
                url = a.value().attr("href").unwrap_or("").to_string();
                if !url.is_empty() {
                    break;
                }
            }
        }
        if title.is_empty() || url.is_empty() {
            continue;
        }
        let poster = Selector::parse("img")
            .ok()
            .and_then(|s| card.select(&s).next())
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
                    .map(str::to_string)
            });
        // Languages: category-* classes first, title text as backup.
        let mut languages: Vec<String> = CAT_LANG
            .captures_iter(class)
            .filter_map(|c| category_language(&c[1]))
            .collect();
        languages.dedup();
        if languages.is_empty() {
            languages = parse_languages(&title);
        }
        let kind = if class.contains("type-series") { "Series" } else { "Movie" };
        out.push(Anime {
            id: url.clone(),
            title,
            url,
            poster,
            languages,
            note: kind.to_string(),
            provider_id: PROVIDER_ID.to_string(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    out.into_iter()
        .filter(|a| seen.insert(a.url.clone()))
        .collect()
}

/// Pull the `CONFIG = {...}` JSON blob out of a dub-player page.
fn extract_config(html: &str) -> Option<(String, HashMap<String, String>, HashMap<String, String>)> {
    let start = html.find("var CONFIG = ")? + "var CONFIG = ".len();
    let bytes = html.as_bytes();
    let mut depth = 0usize;
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let json_str = &html[start..end?];
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let prefix = v.get("prefix")?.as_str()?.to_string();
    let ready: HashMap<String, String> = v
        .get("ready")?
        .as_object()?
        .iter()
        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    let mut names: HashMap<String, String> = HashMap::new();
    if let Some(lang) = v.get("lang").and_then(|l| l.as_object()) {
        for (code, info) in lang {
            if let Some(name) = info.get("name").and_then(|n| n.as_str()) {
                names.insert(code.clone(), name.to_string());
            }
        }
    }
    Some((prefix, ready, names))
}

#[async_trait]
impl Provider for WatchAnimeWorld {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }
    fn name(&self) -> &'static str {
        "WatchAnimeWorld"
    }
    fn base_url(&self) -> &'static str {
        BASE
    }

    async fn trending(&self) -> anyhow::Result<Vec<Anime>> {
        Ok(parse_cards(&get(BASE).await?))
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Anime>> {
        let url = format!("{BASE}/?s={}", query.replace(' ', "+"));
        Ok(parse_cards(&get(&url).await?))
    }

    async fn details(&self, anime: &Anime) -> anyhow::Result<AnimeDetails> {
        let html = get(&anime.url).await?;
        let doc = Html::parse_document(&html);
        let mut synopsis = String::new();
        if let Ok(s) = Selector::parse(".entry-content p, .post-content p") {
            let paras: Vec<String> = doc
                .select(&s)
                .take(3)
                .map(|p| p.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|t| t.len() > 40)
                .collect();
            synopsis = paras.join("\n\n");
            synopsis.truncate(900);
        }
        let mut genres: Vec<String> = Vec::new();
        if let Ok(gs) = Selector::parse("a[rel='category tag'], .cat-links a") {
            for a in doc.select(&gs).take(10) {
                let g: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !g.is_empty() && category_language(&g.to_lowercase()).is_none() && !genres.contains(&g) {
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
        // Movies have a single "episode".
        if anime.note == "Movie" {
            return Ok(vec![Episode {
                number: "1".to_string(),
                title: anime.title.clone(),
                url: anime.url.clone(),
                languages: anime.languages.clone(),
                servers: Vec::new(),
            }]);
        }
        let html = get(&anime.url).await?;
        let doc = Html::parse_document(&html);
        let Ok(a_sel) = Selector::parse("a[href]") else {
            anyhow::bail!("episode list parse failed")
        };
        let mut episodes: Vec<Episode> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for a in doc.select(&a_sel) {
            let href = a.value().attr("href").unwrap_or("");
            let Some(cap) = EP_URL.captures(href) else { continue };
            let season: u32 = cap[1].parse().unwrap_or(1);
            let ep: u32 = cap[2].parse().unwrap_or(0);
            if ep == 0 || !seen.insert((season, ep)) {
                continue;
            }
            let link_title: String =
                a.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let full_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{BASE}{href}")
            };
            episodes.push(Episode {
                number: format!("S{season}E{ep}"),
                title: if link_title.is_empty() {
                    format!("Episode {ep}")
                } else {
                    link_title.chars().take(80).collect()
                },
                url: full_url,
                languages: anime.languages.clone(),
                servers: Vec::new(),
            });
        }
        episodes.sort_by_key(|e| e.number.clone());
        if episodes.is_empty() {
            anyhow::bail!("No episodes found on this page");
        }
        Ok(episodes)
    }

    async fn sources(&self, episode: &Episode) -> anyhow::Result<Vec<StreamSource>> {
        let html = get(&episode.url).await?;
        // Parse the iframes into owned URLs first; the scraper `Html` value
        // is dropped before any `.await` so the future stays `Send`.
        let (dub_url, zephyrix): (Option<String>, Vec<String>) = {
            let doc = Html::parse_document(&html);
            let Ok(if_sel) = Selector::parse("iframe") else {
                anyhow::bail!("no players found")
            };
            let mut dub: Option<String> = None;
            let mut zep: Vec<String> = Vec::new();
            for f in doc.select(&if_sel) {
                let src = f
                    .value()
                    .attr("data-src")
                    .or_else(|| f.value().attr("src"))
                    .unwrap_or("");
                if src.is_empty() {
                    continue;
                }
                let abs = if src.starts_with("http") {
                    src.to_string()
                } else {
                    format!("{BASE}{src}")
                };
                if abs.contains("dub-player") {
                    dub = Some(abs);
                } else if abs.contains("zephyrix") {
                    zep.push(abs);
                }
            }
            (dub, zep)
        };

        let mut out: Vec<StreamSource> = Vec::new();
        for abs in zephyrix {
            out.push(StreamSource {
                label: "Zephyrix HD".to_string(),
                url: abs,
                quality: Some("HD".to_string()),
                kind: SourceKind::EmbedPage,
                headers: vec![("Referer".to_string(), format!("{BASE}/"))],
            });
        }

        // Dub player → one source per audio language.
        if let Some(url) = dub_url {
            if let Ok(cfg_html) = get(&url).await {
                if let Some((prefix, ready, names)) = extract_config(&cfg_html) {
                    let mut codes: Vec<String> = ready.keys().cloned().collect();
                    codes.sort();
                    for code in codes {
                        let token = &ready[&code];
                        let lang = names
                            .get(&code)
                            .cloned()
                            .unwrap_or_else(|| normalise_language(&code));
                        out.push(StreamSource {
                            label: format!("Dub Player · {lang}"),
                            url: format!("{prefix}{token}"),
                            quality: None,
                            kind: SourceKind::EmbedPage,
                            headers: vec![("Referer".to_string(), format!("{BASE}/"))],
                        });
                    }
                }
            }
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
    fn episode_url_parses() {
        let cap = EP_URL
            .captures("https://watchanimeworld.one/episode/fullmetal-alchemist-brotherhood-1x39/")
            .unwrap();
        assert_eq!(&cap[1], "1");
        assert_eq!(&cap[2], "39");
    }

    #[test]
    fn category_language_maps() {
        assert_eq!(category_language("hindi"), Some("Hindi".to_string()));
        assert_eq!(category_language("telugu"), Some("Telugu".to_string()));
        assert_eq!(category_language("anime"), None);
    }

    #[test]
    fn config_extracts_per_language_tokens() {
        let html = r#"<script>(function(){var CONFIG = {"prefix":"https://player.abyssplayer.com/","title":"FMA","season":1,"episode":1,"ready":{"hin":"AAA","eng":"BBB"},"lang":{"hin":{"name":"Hindi","short":"HI"},"eng":{"name":"English","short":"EN"}},"targetOrigin":"https://watchanimeworld.one"};})();</script>"#;
        let (prefix, ready, names) = extract_config(html).unwrap();
        assert_eq!(prefix, "https://player.abyssplayer.com/");
        assert_eq!(ready["hin"], "AAA");
        assert_eq!(names["eng"], "English");
    }
}

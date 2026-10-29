//! Provider abstraction: every anime source implements [`Provider`].
//!
//! A provider turns the site's HTML/JSON into [`Anime`], [`Episode`] and
//! [`StreamSource`] values. The UI only ever talks to this trait, so adding a
//! new site is a single new file.

pub mod animostream;
pub mod hianimeat;
pub mod hianimex;
pub mod watchanimeworld;

use crate::model::{Anime, AnimeDetails, Episode, StreamSource};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn base_url(&self) -> &'static str;

    /// Front page / trending titles (shown on the home screen).
    async fn trending(&self) -> anyhow::Result<Vec<Anime>>;
    /// Full-text search.
    async fn search(&self, query: &str) -> anyhow::Result<Vec<Anime>>;
    /// Detail page for one anime.
    async fn details(&self, anime: &Anime) -> anyhow::Result<AnimeDetails>;
    /// Episode list for one anime.
    async fn episodes(&self, anime: &Anime) -> anyhow::Result<Vec<Episode>>;
    /// Playable streams for one episode.
    async fn sources(&self, episode: &Episode) -> anyhow::Result<Vec<StreamSource>>;
}

/// Every provider the app ships with, in priority order.
/// HiAnimeX is first: it is the only source with *direct* HLS streams
/// (via the zokoanime extractor), so playback actually lands in mpv.
/// HiAnimeAt is pystardust/ani-cli's backend (same HiAnime network,
/// newer `/api/theme/` endpoints) — also with direct ZokoAnime streams.
pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(hianimex::HiAnimeX),
        Box::new(hianimeat::HiAnimeAt),
        Box::new(watchanimeworld::WatchAnimeWorld),
        Box::new(animostream::AnimoStream),
    ]
}

pub fn provider_by_id(id: &str) -> Option<Box<dyn Provider>> {
    all_providers().into_iter().find(|p| p.id() == id)
}

// ---------------------------------------------------------------------------
// Language tag parsing
// ---------------------------------------------------------------------------

/// Matches tokens like "Hindi", "ENG", "Tamil", "JAP", "Telugu", "Malayalam",
/// "Dual Audio", "Multi Audio" inside release titles such as
/// "Dual/Multi Audio [Hindi ORG, ENG ] WEB-DL 720p HEVC".
static LANG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(hindi|english|eng|japanese|jap|tamil|telugu|malayalam|kannada|bengali|marathi|punjabi|urdu|french|spanish|german|portuguese|italian|russian|arabic|korean|chinese)\b",
    )
    .unwrap()
});

/// Normalise a raw language token to a canonical display name.
pub fn normalise_language(token: &str) -> String {
    match token.to_ascii_lowercase().as_str() {
        "eng" => "English".to_string(),
        "jap" => "Japanese".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

/// Extract ordered, de-duplicated language names from a release title.
pub fn parse_languages(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cap in LANG_RE.captures_iter(title) {
        let lang = normalise_language(&cap[1]);
        if !lang.is_empty() && !out.contains(&lang) {
            out.push(lang);
        }
    }
    out
}

/// A shared reqwest client with a real browser User-Agent (many anime sites
/// 403 plain library UAs).
///
/// Besides the built-in webpki roots, this also honours the `SSL_CERT_FILE`
/// environment variable: sandboxed / corporate networks that MITM TLS with a
/// custom CA (curl-style) keep working without disabling verification.
pub fn http_client() -> reqwest::Client {
    static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
        let mut builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(25));
        if let Ok(path) = std::env::var("SSL_CERT_FILE") {
            if let Ok(pem) = std::fs::read(&path) {
                let mut reader = std::io::BufReader::new(&pem[..]);
                for cert in rustls_pemfile::certs(&mut reader).flatten() {
                    if let Ok(c) = reqwest::Certificate::from_der(&cert) {
                        builder = builder.add_root_certificate(c);
                    }
                }
            }
        }
        builder.build().expect("reqwest client")
    });
    CLIENT.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dual_audio_title() {
        let langs = parse_languages("Attack On Titan Dual/Multi Audio [Hindi ORG, ENG ] WEB-DL 720p HEVC");
        assert_eq!(langs, vec!["Hindi", "English"]);
    }

    #[test]
    fn parses_japanese_and_dedupes() {
        let langs = parse_languages("Naruto [Japanese] Subbed JAP 1080p");
        assert_eq!(langs, vec!["Japanese"]);
    }

    #[test]
    fn empty_when_no_language() {
        assert!(parse_languages("Some Random Show 720p").is_empty());
    }
}

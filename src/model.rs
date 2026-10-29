//! Core data model shared by every provider.

use serde::{Deserialize, Serialize};

/// A single anime series / season as returned by a provider search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Anime {
    /// Provider-local identifier (slug, numeric id, ...).
    pub id: String,
    pub title: String,
    /// Canonical page URL on the provider site.
    pub url: String,
    pub poster: Option<String>,
    /// Languages the release is available in, e.g. ["Hindi", "English", "Japanese"].
    /// Shown to the user *before* they pick an episode.
    pub languages: Vec<String>,
    /// Short free-text note, e.g. "S01 EP 1-16 Added".
    pub note: String,
    /// Which provider this entry came from.
    pub provider_id: String,
}

impl Anime {
    /// Human friendly one-liner for lists: "Title  [Hindi, English]".
    pub fn display_title(&self) -> String {
        if self.languages.is_empty() {
            self.title.clone()
        } else {
            format!("{}  [{}]", self.title, self.languages.join(", "))
        }
    }
}

/// Extra metadata shown on the detail screen.
#[derive(Debug, Clone, Default)]
pub struct AnimeDetails {
    pub anime: Anime,
    pub synopsis: String,
    pub genres: Vec<String>,
    pub status: String,
    pub year: String,
}

/// One episode of an anime.
#[derive(Debug, Clone)]
pub struct Episode {
    pub number: String,
    pub title: String,
    pub url: String,
    /// Languages available for *this* episode (may differ per episode).
    pub languages: Vec<String>,
    /// Playback endpoints discovered for this episode. [`Provider::sources`]
    /// turns these into playable [`StreamSource`] values.
    pub servers: Vec<ServerRef>,
}

/// A named playback endpoint for an episode (a server button, a per-language
/// player link, ...). The provider resolves these into [`StreamSource`]s.
#[derive(Debug, Clone)]
pub struct ServerRef {
    /// e.g. "Server 1", "Hindi", "SUB · HD-2".
    pub name: String,
    pub url: String,
    /// Language of this server's audio, when known.
    pub language: Option<String>,
}

/// One playable stream for an episode.
#[derive(Debug, Clone)]
pub struct StreamSource {
    /// e.g. "Server 1 · 720p", "GDrive", "HLS".
    pub label: String,
    /// Direct media URL (.mp4 / .m3u8) when known.
    pub url: String,
    pub quality: Option<String>,
    pub kind: SourceKind,
    /// Extra HTTP headers the player must send (Referer, ...).
    pub headers: Vec<(String, String)>,
}

/// How the stream URL should be treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Direct video file (.mp4, .mkv, ...).
    Direct,
    /// HLS playlist (.m3u8). mpv/vlc handle it natively.
    Hls,
    /// An embed/watch page — opened in the browser as a fallback.
    EmbedPage,
}

impl StreamSource {
    pub fn referer(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("referer"))
            .map(|(_, v)| v.as_str())
    }
}

/// One played episode, persisted to disk and shown on the home screen
/// ("Recently played") instead of trending titles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub anime_id: String,
    pub anime_title: String,
    pub anime_url: String,
    pub provider_id: String,
    pub episode_number: String,
    pub episode_title: String,
    pub played_at_unix: i64,
}

impl HistoryEntry {
    /// Row shown on the home screen.
    pub fn as_anime(&self) -> Anime {
        Anime {
            id: self.anime_id.clone(),
            title: self.anime_title.clone(),
            url: self.anime_url.clone(),
            poster: None,
            languages: vec![],
            note: format!("EP {}", self.episode_number),
            provider_id: self.provider_id.clone(),
        }
    }
}

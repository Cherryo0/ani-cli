//! Watch-history persistence: ~/.config/ani-cli/history.json

use crate::model::HistoryEntry;
use std::path::PathBuf;

fn history_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/ani-cli/history.json"))
}

pub fn load_history() -> Vec<HistoryEntry> {
    let Some(path) = history_path() else {
        return Vec::new();
    };
    load_history_from(&path)
}

pub fn save_history(entries: &[HistoryEntry]) {
    let Some(path) = history_path() else {
        return;
    };
    save_history_to(&path, entries);
}

pub fn load_history_from(path: &std::path::Path) -> Vec<HistoryEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_history_to(path: &std::path::Path, entries: &[HistoryEntry]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ani-cli-test-{name}-{}.json", std::process::id()));
        p
    }

    fn entry(n: &str) -> HistoryEntry {
        HistoryEntry {
            anime_id: "43".into(),
            anime_title: "Naruto".into(),
            anime_url: "https://example.com/x".into(),
            provider_id: "hianimex".into(),
            episode_number: n.into(),
            episode_title: format!("Episode {n}"),
            played_at_unix: 1_700_000_000,
        }
    }

    #[test]
    fn history_roundtrips_through_disk() {
        let path = tmp_path("roundtrip");
        let entries = vec![entry("1"), entry("2")];
        save_history_to(&path, &entries);
        let loaded = load_history_from(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].episode_number, "1");
        assert_eq!(loaded[1].anime_title, "Naruto");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_loads_empty() {
        let path = tmp_path("missing");
        let _ = std::fs::remove_file(&path);
        assert!(load_history_from(&path).is_empty());
    }
}

//! Launching external video players.
//!
//! The TUI never plays video itself — it resolves a stream URL and hands it
//! to the user's favourite player. mpv is the default (it handles direct
//! files, HLS and custom Referer headers); everything else is best-effort.

use crate::model::{SourceKind, StreamSource};
use std::process::Command;

/// Players the app knows how to drive, in auto-detect priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKind {
    Mpv,
    Vlc,
    Iina,
    Ffplay,
    Celluloid,
    Haruna,
}

impl PlayerKind {
    pub fn name(&self) -> &'static str {
        match self {
            PlayerKind::Mpv => "mpv",
            PlayerKind::Vlc => "vlc",
            PlayerKind::Iina => "iina",
            PlayerKind::Ffplay => "ffplay",
            PlayerKind::Celluloid => "celluloid",
            PlayerKind::Haruna => "haruna",
        }
    }

    pub fn binary(&self) -> &'static str {
        match self {
            PlayerKind::Mpv => "mpv",
            // `vlc` CLI exists on macOS inside the .app bundle; fall back to `open`.
            PlayerKind::Vlc => "vlc",
            PlayerKind::Iina => "iina",
            PlayerKind::Ffplay => "ffplay",
            PlayerKind::Celluloid => "celluloid",
            PlayerKind::Haruna => "haruna",
        }
    }

    pub fn from_name(name: &str) -> Option<PlayerKind> {
        match name.to_ascii_lowercase().as_str() {
            "mpv" => Some(PlayerKind::Mpv),
            "vlc" => Some(PlayerKind::Vlc),
            "iina" => Some(PlayerKind::Iina),
            "ffplay" => Some(PlayerKind::Ffplay),
            "celluloid" => Some(PlayerKind::Celluloid),
            "haruna" => Some(PlayerKind::Haruna),
            _ => None,
        }
    }

    /// All known players, highest priority first.
    pub fn all() -> Vec<PlayerKind> {
        vec![
            PlayerKind::Mpv,
            PlayerKind::Vlc,
            PlayerKind::Iina,
            PlayerKind::Ffplay,
            PlayerKind::Celluloid,
            PlayerKind::Haruna,
        ]
    }
}

/// Is this player installed (binary on PATH, or macOS .app present)?
pub fn is_available(kind: PlayerKind) -> bool {
    if which_binary(kind.binary()) {
        return true;
    }
    // macOS .app bundles live outside PATH.
    #[cfg(target_os = "macos")]
    {
        let app = match kind {
            PlayerKind::Vlc => "/Applications/VLC.app",
            PlayerKind::Iina => "/Applications/IINA.app",
            _ => "",
        };
        if !app.is_empty() && std::path::Path::new(app).exists() {
            return true;
        }
    }
    false
}

fn which_binary(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .any(|p| p.join(bin).is_file() || p.join(format!("{bin}.exe")).is_file())
        })
        .unwrap_or(false)
}

/// Auto-detect the best installed player.
pub fn detect() -> Option<PlayerKind> {
    PlayerKind::all().into_iter().find(|k| is_available(*k))
}

/// Build the command line for `player` to play `source`.
pub fn build_command(
    player: PlayerKind,
    source: &StreamSource,
    title: &str,
) -> anyhow::Result<Command> {
    let referer = source.referer().map(str::to_string);
    let url = source.url.clone();

    let mut cmd = match player {
        PlayerKind::Mpv => {
            let mut c = Command::new("mpv");
            c.arg(format!("--force-media-title={title}"));
            if let Some(r) = &referer {
                c.arg(format!("--http-header-fields=Referer: {r}"));
            }
            if source.kind == SourceKind::Hls {
                c.arg("--hls-bitrate=max");
            }
            c.arg("--no-terminal");
            c
        }
        PlayerKind::Vlc => {
            let mut c = Command::new("vlc");
            if let Some(r) = &referer {
                c.arg(format!("--http-referrer={r}"));
            }
            c.arg("--meta-title").arg(title);
            c
        }
        PlayerKind::Iina => {
            let mut c = Command::new("iina");
            c.arg("--no-stdin").arg("--keep-running");
            c
        }
        PlayerKind::Ffplay => {
            let mut c = Command::new("ffplay");
            c.arg("-autoexit").arg("-window_title").arg(title);
            if let Some(r) = &referer {
                c.arg("-headers").arg(format!("Referer: {r}\r\n"));
            }
            c
        }
        PlayerKind::Celluloid => {
            let mut c = Command::new("celluloid");
            if let Some(r) = &referer {
                c.arg(format!("--http-header-fields=Referer: {r}"));
            }
            c
        }
        PlayerKind::Haruna => {
            let c = Command::new("haruna");
            c
        }
    };
    cmd.arg(url);
    Ok(cmd)
}

/// Play `source` in `player`, blocking until the player exits.
pub fn play(player: PlayerKind, source: &StreamSource, title: &str) -> anyhow::Result<()> {
    if source.kind == SourceKind::EmbedPage {
        open_in_browser(&source.url)?;
        return Ok(());
    }
    let mut cmd = build_command(player, source, title)?;
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("{} exited with status {status}", player.name());
    }
    Ok(())
}

fn open_in_browser(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).status()?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).status()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", url]).status()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StreamSource;

    fn sample() -> StreamSource {
        StreamSource {
            label: "HD".into(),
            url: "https://cdn.example/v.mp4".into(),
            quality: Some("720p".into()),
            kind: SourceKind::Direct,
            headers: vec![("Referer".into(), "https://animostream.xyz/".into())],
        }
    }

    #[test]
    fn mpv_gets_referer_header() {
        let cmd = build_command(PlayerKind::Mpv, &sample(), "Naruto EP 1").unwrap();
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a.contains("Referer: https://animostream.xyz/")));
        assert!(args.iter().any(|a| a.contains("Naruto EP 1")));
    }

    #[test]
    fn from_name_roundtrip() {
        assert_eq!(PlayerKind::from_name("MPV"), Some(PlayerKind::Mpv));
        assert_eq!(PlayerKind::from_name("vlc"), Some(PlayerKind::Vlc));
        assert_eq!(PlayerKind::from_name("nope"), None);
    }
}

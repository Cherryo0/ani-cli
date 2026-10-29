//! TUI render tests (tui-ux-audit skill, step 2): every screen in every
//! state, asserting the text the user decides on. Deterministic fixtures —
//! no network.

use ani_cli::{
    app::{App, Screen},
    model::{Anime, AnimeDetails, Episode, SourceKind, StreamSource},
    ui,
};
use ratatui::{backend::TestBackend, Terminal};

fn render(app: &mut App) -> String {
    let backend = TestBackend::new(100, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf.get(x, y).symbol());
        }
        out.push('\n');
    }
    out
}

fn sample_anime() -> Anime {
    Anime {
        id: "43".into(),
        title: "Naruto: Shippuden the Movie".into(),
        url: "https://hianimex.com.in/watch/naruto-shippuden-the-movie-vv6fp-43".into(),
        poster: None,
        languages: vec!["Japanese".into(), "English".into()],
        note: "".into(),
        provider_id: "hianimex".into(),
    }
}

fn sample_episode() -> Episode {
    Episode {
        number: "1".into(),
        title: "Episode 1".into(),
        url: "https://hianimex.com.in/watch/x?ep=76".into(),
        languages: vec!["Japanese".into()],
        servers: vec![],
    }
}

#[test]
fn home_empty_shows_logo_search_pill_and_hints() {
    let mut app = App::new(None, None);
    let text = render(&mut app);
    assert!(text.contains("Search anime"), "search placeholder missing");
    // Provider pill shows the active provider, MovieBox-Tui style.
    assert!(text.contains("HiAnimeX"), "provider pill missing");
    assert!(text.contains("Ctrl+P"), "provider switch hint missing");
    assert!(text.contains("search"), "bottom bar hints missing");
    assert!(text.contains("quit"), "bottom bar hints missing");
}

#[test]
fn home_populated_shows_titles_and_language_badges() {
    let mut app = App::new(None, None);
    app.items = vec![sample_anime()];
    let text = render(&mut app);
    assert!(text.contains("Naruto: Shippuden the Movie"));
    assert!(text.contains("Japanese"), "language badge missing");
    assert!(text.contains("English"), "language badge missing");
}

#[test]
fn home_loading_spinner_visible() {
    let mut app = App::new(None, None);
    app.loading = true;
    let text = render(&mut app);
    assert!(text.contains("Loading"), "loading indicator missing");
}

#[test]
fn home_error_status_visible() {
    let mut app = App::new(None, None);
    app.status = "Error: No playable sources found for this episode".into();
    let text = render(&mut app);
    assert!(
        text.contains("No playable sources"),
        "error status not visible in footer"
    );
}

#[test]
fn home_input_mode_echoes_query() {
    let mut app = App::new(None, None);
    app.input_mode = true;
    app.search_input = "naru".into();
    let text = render(&mut app);
    assert!(text.contains("naru"), "typed query not echoed");
}

#[test]
fn detail_shows_languages_line_and_episodes() {
    let mut app = App::new(None, None);
    let anime = sample_anime();
    app.screen = Screen::Detail;
    app.detail = Some(AnimeDetails {
        anime: anime.clone(),
        synopsis: "A ninja movie.".into(),
        genres: vec!["Action".into()],
        status: "".into(),
        year: "".into(),
    });
    app.episodes = vec![sample_episode()];
    let text = render(&mut app);
    assert!(text.contains("Languages:"), "languages header missing");
    assert!(text.contains("Japanese"), "language value missing");
    assert!(text.contains("EP 1"), "episode row missing");
    // Languages must appear before the episode list in the layout.
    let lang_pos = text.find("Languages:").unwrap();
    let ep_pos = text.find("EP 1").unwrap();
    assert!(lang_pos < ep_pos, "languages must be shown before episodes");
}

#[test]
fn detail_unknown_languages_says_unknown() {
    let mut app = App::new(None, None);
    let mut anime = sample_anime();
    anime.languages.clear();
    app.screen = Screen::Detail;
    app.detail = Some(AnimeDetails {
        anime,
        synopsis: String::new(),
        genres: vec![],
        status: String::new(),
        year: String::new(),
    });
    app.episodes = vec![];
    let text = render(&mut app);
    assert!(text.contains("unknown"), "should say 'unknown', not blank");
}

#[test]
fn sources_show_kind_and_quality_badges() {
    let mut app = App::new(None, None);
    app.screen = Screen::Sources;
    app.sources = vec![
        StreamSource {
            label: "English (Dub) · Direct".into(),
            url: "https://cdn.example/x.m3u8".into(),
            quality: Some("HD".into()),
            kind: SourceKind::Hls,
            headers: vec![],
        subtitle_url: None,
        },
        StreamSource {
            label: "Japanese (Sub) · Server 1".into(),
            url: "https://megaplay.buzz/stream/x".into(),
            quality: None,
            kind: SourceKind::EmbedPage,
            headers: vec![],
        subtitle_url: None,
        },
    ];
    let text = render(&mut app);
    assert!(text.contains("[hls]"), "hls badge missing");
    assert!(text.contains("[browser]"), "browser badge missing");
    assert!(text.contains("HD"), "quality badge missing");
    assert!(text.contains("English (Dub)"), "language label missing");
}

#[test]
fn help_overlay_shows_keybindings() {
    let mut app = App::new(None, None);
    app.screen = Screen::Help;
    let text = render(&mut app);
    assert!(text.contains("keybindings"), "help title missing");
    assert!(text.contains("/"), "search key missing");
    assert!(text.contains("switch provider"), "provider key missing");
    assert!(text.contains("switch video player"), "player key missing");
}

#[test]
fn footer_hints_change_per_screen() {
    let mut app = App::new(None, None);
    let home = render(&mut app);
    assert!(home.contains("provider"), "home hints wrong");
    app.screen = Screen::Detail;
    app.detail = Some(AnimeDetails {
        anime: sample_anime(),
        synopsis: String::new(),
        genres: vec![],
        status: String::new(),
        year: String::new(),
    });
    app.episodes = vec![sample_episode()];
    let detail = render(&mut app);
    assert!(detail.contains("play episode"), "detail hints wrong");
}

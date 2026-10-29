//! Renders ani-cli screens as plain text for visual review.

use ani_cli::{
    app::{App, Screen},
    model::{Anime, AnimeDetails, Episode},
    ui,
};
use ratatui::{backend::TestBackend, Terminal};

fn shot(app: &mut App, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.get(x, y).symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn anime(title: &str, langs: &[&str]) -> Anime {
    Anime {
        id: "1".into(),
        title: title.into(),
        url: "https://example.com".into(),
        poster: None,
        languages: langs.iter().map(|s| s.to_string()).collect(),
        note: "TV · 24 eps".into(),
        provider_id: "hianimex".into(),
    }
}

fn main() {
    // 1. Landing (what you see on boot)
    let mut app = App::new(None, None);
    println!("=== LANDING (boot) ===");
    println!("{}", shot(&mut app, 80, 24));

    // 2. Results
    app.items = vec![
        anime("Naruto: Shippuden", &["Japanese", "English", "Hindi"]),
        anime("One Piece", &["Japanese", "English"]),
        anime("Demon Slayer", &["Japanese"]),
    ];
    println!("=== RESULTS ===");
    println!("{}", shot(&mut app, 80, 24));

    // 3. Detail
    app.screen = Screen::Detail;
    app.detail = Some(AnimeDetails {
        anime: anime("Naruto: Shippuden", &["Japanese", "English", "Hindi"]),
        synopsis: "Naruto Uzumaki, a mischievous adolescent ninja, struggles as he searches for recognition and dreams of becoming the Hokage.".into(),
        genres: vec!["Action".into(), "Adventure".into()],
        status: "Completed".into(),
        year: "2007".into(),
    });
    app.episodes = vec![
        Episode { number: "1".into(), title: "The Girl with the Blue Hair".into(), url: String::new(), languages: vec!["Japanese".into()], servers: vec![] },
        Episode { number: "2".into(), title: "The Akatsuki Makes Its Move".into(), url: String::new(), languages: vec!["Japanese".into(), "English".into()], servers: vec![] },
    ];
    println!("=== DETAIL ===");
    println!("{}", shot(&mut app, 80, 24));
}

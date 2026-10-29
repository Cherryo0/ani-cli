//! Live smoke tests — hit the real sites. Ignored by default because they
//! need network access; run with `cargo test -- --ignored`.

use ani_cli::providers;

#[tokio::test]
#[ignore]
async fn hianimex_search_and_episodes() {
    let p = providers::provider_by_id("hianimex").unwrap();
    let results = p.search("naruto").await.expect("search works");
    assert!(!results.is_empty(), "expected results for 'naruto'");
    println!("first hit: {} [{}]", results[0].title, results[0].languages.join(","));
    let eps = p.episodes(&results[0]).await.expect("episodes work");
    assert!(!eps.is_empty(), "expected episodes");
    println!("episodes: {}", eps.len());
}

#[tokio::test]
#[ignore]
async fn hianimex_server_chain() {
    let p = providers::provider_by_id("hianimex").unwrap();
    let results = p.search("naruto").await.unwrap();
    let eps = p.episodes(&results[0]).await.unwrap();
    let sources = p.sources(&eps[0]).await.expect("sources work");
    assert!(!sources.is_empty(), "expected sources");
    for s in &sources {
        println!("source: {} -> {}", s.label, s.url);
    }
}

#[tokio::test]
#[ignore]
async fn zokoanime_extractor_yields_direct_hls() {
    let direct = ani_cli::extractors::resolve_zokoanime(
        "https://zokoanime.video/stream/mal/2472/1/sub?color=ffbade",
    )
    .await
    .expect("extractor works");
    assert!(
        direct.url.ends_with(".m3u8"),
        "expected an m3u8 url, got {}",
        direct.url
    );
    println!("direct: {}", direct.url);
}

#[tokio::test]
#[ignore]
async fn watchanimeworld_search() {
    let p = providers::provider_by_id("watchanimeworld").unwrap();
    let results = p.search("naruto").await.expect("search works");
    assert!(!results.is_empty(), "expected results for 'naruto'");
    println!(
        "first hit: {} [{}] ({})",
        results[0].title,
        results[0].languages.join(","),
        results[0].note
    );
}

#[tokio::test]
#[ignore]
async fn hianimeat_full_chain_with_subtitles() {
    // pystardust/ani-cli's backend: search -> episodes -> servers ->
    // direct HLS + subtitles, verified playable.
    let p = providers::provider_by_id("hianimeat").unwrap();
    let results = p.search("naruto").await.expect("search works");
    assert!(!results.is_empty(), "expected results for 'naruto'");
    println!("first hit: {} [{}]", results[0].title, results[0].languages.join(","));
    let eps = p.episodes(&results[0]).await.expect("episodes work");
    assert!(!eps.is_empty(), "expected episodes");
    let sources = p.sources(&eps[0]).await.expect("sources work");
    assert!(!sources.is_empty(), "expected sources");
    // Direct sources must sort first so playback stays in mpv.
    assert!(
        matches!(
            sources[0].kind,
            ani_cli::model::SourceKind::Hls | ani_cli::model::SourceKind::Direct
        ),
        "first source should be playable in-player, got {:?}",
        sources[0].kind
    );
    let direct = sources.iter().find(|s| s.subtitle_url.is_some());
    println!(
        "direct with subtitles: {}",
        direct.map(|s| s.label.as_str()).unwrap_or("<none>")
    );
    // The direct stream URL must actually serve a playlist (would-be playback).
    let m3u8 = &sources[0].url;
    assert!(m3u8.ends_with(".m3u8"), "expected m3u8, got {m3u8}");
    let body = providers::http_client()
        .get(m3u8)
        .header("Referer", "https://zokoanime.video/")
        .send()
        .await
        .expect("playlist fetch")
        .error_for_status()
        .expect("playlist 200")
        .text()
        .await
        .expect("playlist body");
    assert!(body.contains("#EXTM3U"), "not a valid HLS playlist");
    println!("playlist OK ({} bytes)", body.len());
}

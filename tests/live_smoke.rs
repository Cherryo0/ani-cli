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

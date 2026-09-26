//! Live contract checks. Run with `cargo test --test live_e2e -- --ignored --test-threads=1`.

use std::sync::Arc;

use chrono::{Duration, Local};
use quick_repertoire::cinema::browser::ChromiumHtmlRenderer;
use quick_repertoire::cinema::cinema_city::CinemaCity;
use quick_repertoire::cinema::common::MISSING_DATA_LABEL;
use quick_repertoire::cinema::helios::{
    DEFAULT_HELIOS_BASE_URL, DEFAULT_HELIOS_VENUES_URL, Helios,
};
use quick_repertoire::cinema::multikino::{DEFAULT_MULTIKINO_BASE_URL, Multikino};
use quick_repertoire::cinema::registry::CinemaChainClient;
use quick_repertoire::config::{
    DEFAULT_CINEMA_CITY_REPERTOIRE_URL, DEFAULT_CINEMA_CITY_VENUES_LIST_URL,
};
use quick_repertoire::domain::{Repertoire, TmdbLookupMovie};
use quick_repertoire::tmdb::{ReqwestTmdbClient, TmdbService};

fn tomorrow() -> String {
    (Local::now().date_naive() + Duration::days(1)).format("%Y-%m-%d").to_string()
}

fn present(value: &str) -> bool {
    !value.trim().is_empty() && value != MISSING_DATA_LABEL
}

fn assert_coverage(
    chain: &str,
    movies: &[Repertoire],
    field: &str,
    value: impl Fn(&Repertoire) -> bool,
) {
    let filled = movies.iter().filter(|movie| value(movie)).count();
    assert!(
        filled * 2 >= movies.len(),
        "{chain}: {field} populated for {filled}/{} movies",
        movies.len()
    );
}

fn assert_repertoire(chain: &str, movies: &[Repertoire], check_genres: bool) {
    assert!(!movies.is_empty(), "{chain}: no movies for tomorrow");
    assert_coverage(chain, movies, "title", |movie| present(&movie.title));
    if check_genres {
        assert_coverage(chain, movies, "genres", |movie| present(&movie.genres));
    }
    assert_coverage(chain, movies, "runtime", |movie| present(&movie.play_length));
    assert!(
        movies.iter().any(|movie| {
            movie.play_details.iter().any(|detail| {
                detail
                    .play_times
                    .iter()
                    .any(|time| present(&time.value) && time.url.as_deref().is_some_and(present))
            })
        }),
        "{chain}: no bookable showtimes"
    );
}

async fn cinema_city_repertoire() -> Vec<Repertoire> {
    let client = CinemaCity::new(
        DEFAULT_CINEMA_CITY_REPERTOIRE_URL.to_string(),
        DEFAULT_CINEMA_CITY_VENUES_LIST_URL.to_string(),
        Arc::new(ChromiumHtmlRenderer),
    );
    let venues = client.fetch_venues().await.expect("Cinema City venues request failed");
    let venue = venues
        .iter()
        .find(|venue| venue.venue_name.contains("Wroclavia"))
        .expect("Cinema City Wroclavia missing from venue list");
    client
        .fetch_repertoire(&tomorrow(), venue)
        .await
        .expect("Cinema City repertoire request failed")
}

#[tokio::test]
#[ignore = "makes live Cinema City requests"]
async fn cinema_city_live_repertoire_has_core_columns() {
    let movies = cinema_city_repertoire().await;
    assert_repertoire("Cinema City", &movies, true);
    assert_coverage("Cinema City", &movies, "original language", |movie| {
        present(&movie.original_language)
    });
    assert_coverage("Cinema City", &movies, "screening language", |movie| {
        movie.play_details.iter().any(|detail| present(&detail.play_language))
    });
}

#[tokio::test]
#[ignore = "makes live Helios requests"]
async fn helios_live_repertoire_has_core_columns() {
    let client = Helios::new(
        DEFAULT_HELIOS_BASE_URL,
        DEFAULT_HELIOS_VENUES_URL,
        Arc::new(ChromiumHtmlRenderer),
    );
    let venues = client.fetch_venues().await.expect("Helios venues request failed");
    let venue = venues
        .iter()
        .find(|venue| venue.venue_id == "lodz/kino-helios")
        .expect("Helios Łódź missing from venue list");
    let movies = client
        .fetch_repertoire(&tomorrow(), venue)
        .await
        .expect("Helios repertoire request failed");
    assert_repertoire("Helios", &movies, true);
    assert_coverage("Helios", &movies, "screening language", |movie| {
        movie.play_details.iter().any(|detail| present(&detail.play_language))
    });
}

#[tokio::test]
#[ignore = "makes live Multikino requests"]
async fn multikino_live_repertoire_has_core_columns() {
    let client = Multikino::new(DEFAULT_MULTIKINO_BASE_URL);
    let venues = client.fetch_venues().await.expect("Multikino venues request failed");
    let venue = venues
        .iter()
        .find(|venue| venue.venue_name.contains("Złote Tarasy"))
        .expect("Multikino Złote Tarasy missing from venue list");
    let movies = client
        .fetch_repertoire(&tomorrow(), venue)
        .await
        .expect("Multikino repertoire request failed");
    assert_repertoire("Multikino", &movies, false);
    assert_coverage("Multikino", &movies, "screening language", |movie| {
        movie.play_details.iter().any(|detail| present(&detail.play_language))
    });
}

#[tokio::test]
#[ignore = "requires TMDB_ACCESS_TOKEN and makes live Cinema City and TMDB requests"]
async fn tmdb_live_matches_current_cinema_movies() {
    let token = std::env::var("TMDB_ACCESS_TOKEN").expect("set TMDB_ACCESS_TOKEN for live tests");
    assert!(!token.trim().is_empty(), "TMDB_ACCESS_TOKEN is empty");
    let movies = cinema_city_repertoire().await;
    assert_repertoire("Cinema City", &movies, true);
    let lookups = movies.iter().take(12).map(TmdbLookupMovie::from).collect::<Vec<_>>();
    let details = ReqwestTmdbClient::new()
        .expect("TMDB client setup failed")
        .get_movie_ratings_and_summaries(&lookups, &token)
        .await
        .expect("TMDB requests failed");
    let rated = details.values().filter(|detail| !detail.rating.trim().is_empty()).count();
    assert!(
        rated * 2 >= lookups.len(),
        "TMDB ratings populated for {rated}/{} current movies",
        lookups.len()
    );
    let summarized = details.values().filter(|detail| present(&detail.summary)).count();
    assert!(
        summarized * 2 >= lookups.len(),
        "TMDB summaries populated for {summarized}/{} current movies",
        lookups.len()
    );
}

mod support;

use std::collections::HashMap;

use httpmock::Method::GET;
use httpmock::MockServer;
use quick_repertoire::domain::TmdbMovieDetails;
use quick_repertoire::tmdb::{
    ReqwestTmdbClient, TmdbService, ensure_single_result, parse_movie_rating, parse_movie_summary,
};
use serde_json::json;

#[tokio::test]
async fn get_movie_ratings_and_summaries_returns_correct_tmdb_movie_details() {
    let server = MockServer::start_async().await;
    let payloads = HashMap::from([
        (
            "Garfield".to_string(),
            json!({
                "results": [{
                    "vote_average": 6.717,
                    "vote_count": 184,
                    "overview": "Garfield jest najbardziej znanym kotem na świecie."
                }]
            }),
        ),
        (
            "Furiosa: Saga Mad Max".to_string(),
            json!({
                "results": [{
                    "vote_average": 7.631,
                    "vote_count": 967,
                    "overview": "Kiedy świat upada."
                }]
            }),
        ),
    ]);

    for (movie, payload) in &payloads {
        let payload = payload.clone();
        server
            .mock_async(|when, then| {
                when.method(GET).path("/search/movie").query_param("query", movie.as_str());
                then.status(200).json_body(payload);
            })
            .await;
    }

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &["Garfield".to_string(), "Furiosa: Saga Mad Max".to_string()],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([
            (
                "Garfield".to_string(),
                TmdbMovieDetails {
                    rating: "6.717/10\n(głosy: 184)".to_string(),
                    summary: "Garfield jest najbardziej znanym kotem na świecie.".to_string(),
                }
            ),
            (
                "Furiosa: Saga Mad Max".to_string(),
                TmdbMovieDetails {
                    rating: "7.631/10\n(głosy: 967)".to_string(),
                    summary: "Kiedy świat upada.".to_string(),
                }
            ),
        ])
    );
}

#[test]
fn tmdb_parsing_helpers_handle_missing_or_ambiguous_results() {
    let valid_payload = json!({
        "results": [{
            "vote_average": 8.5,
            "vote_count": 2000,
            "overview": "A tense mystery."
        }]
    });
    let invalid_payload = json!({"results": []});

    assert!(ensure_single_result(&valid_payload));
    assert_eq!(parse_movie_rating(&valid_payload), "8.5/10\n(głosy: 2000)");
    assert_eq!(parse_movie_summary(&valid_payload), "A tense mystery.");
    assert!(!ensure_single_result(&invalid_payload));
    assert_eq!(parse_movie_rating(&invalid_payload), "0.0/10");
    assert_eq!(parse_movie_summary(&invalid_payload), "Brak opisu filmu.");
}

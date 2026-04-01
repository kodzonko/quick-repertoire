use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use httpmock::Method::GET;
use httpmock::MockServer;
use quick_repertoire::domain::{MovieLookupMetadata, TmdbLookupMovie, TmdbMovieDetails};
use quick_repertoire::error::AppError;
use quick_repertoire::retry::RetryPolicy;
use quick_repertoire::tmdb::{ReqwestTmdbClient, TmdbService};
use serde_json::json;

fn lookup_movie(title: &str) -> TmdbLookupMovie {
    lookup_movie_with_metadata(title, MovieLookupMetadata::default())
}

fn lookup_movie_with_metadata(title: &str, metadata: MovieLookupMetadata) -> TmdbLookupMovie {
    let lookup_key = metadata.cinema_city_film_id.clone().unwrap_or_else(|| title.to_string());
    TmdbLookupMovie { lookup_key, title: title.to_string(), lookup_metadata: metadata }
}

#[allow(clippy::too_many_arguments)]
fn tmdb_search_result(
    id: u64,
    title: &str,
    original_title: &str,
    release_date: &str,
    original_language: &str,
    vote_average: f64,
    vote_count: u32,
    overview: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "original_title": original_title,
        "release_date": release_date,
        "original_language": original_language,
        "vote_average": vote_average,
        "vote_count": vote_count,
        "overview": overview,
    })
}

#[allow(clippy::too_many_arguments)]
fn tmdb_movie_details(
    id: u64,
    title: &str,
    original_title: &str,
    release_date: &str,
    original_language: &str,
    runtime: u16,
    vote_average: f64,
    vote_count: u32,
    overview: &str,
    genres: &[&str],
    countries: &[(&str, &str)],
    directors: &[&str],
    cast: &[&str],
) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "original_title": original_title,
        "release_date": release_date,
        "original_language": original_language,
        "runtime": runtime,
        "vote_average": vote_average,
        "vote_count": vote_count,
        "overview": overview,
        "genres": genres.iter().map(|name| json!({"name": name})).collect::<Vec<_>>(),
        "production_countries": countries
            .iter()
            .map(|(code, name)| json!({"iso_3166_1": code, "name": name}))
            .collect::<Vec<_>>(),
        "credits": {
            "crew": directors
                .iter()
                .map(|name| json!({"job": "Director", "name": name}))
                .collect::<Vec<_>>(),
            "cast": cast
                .iter()
                .map(|name| json!({"name": name}))
                .collect::<Vec<_>>(),
        }
    })
}

fn json_response(
    status_code: u16,
    reason_phrase: &str,
    body: serde_json::Value,
    extra_headers: &[(&str, &str)],
) -> String {
    let body = body.to_string();
    let mut response = format!(
        "HTTP/1.1 {status_code} {reason_phrase}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );

    for (name, value) in extra_headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }

    response.push_str("\r\n");
    response.push_str(&body);
    response
}

fn start_sequenced_http_server(
    responses: Vec<String>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().expect("test server should expose its address");
    let recorded_requests = Arc::new(Mutex::new(Vec::new()));
    let recorded_requests_handle = recorded_requests.clone();

    let server = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("test server should accept a request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];

            loop {
                let bytes_read = stream.read(&mut buffer).expect("request should be readable");
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes_read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            recorded_requests_handle
                .lock()
                .expect("recorded requests lock poisoned")
                .push(String::from_utf8(request).expect("request should be valid utf-8"));
            stream.write_all(response.as_bytes()).expect("response should be writable");
            stream.flush().expect("response should flush");
        }
    });

    (format!("http://{address}"), recorded_requests, server)
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_ranks_ambiguous_localized_titles_with_metadata() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/search/movie")
                .query_param("query", "Tajny agent")
                .query_param("include_adult", "true")
                .query_param("language", "pl-PL")
                .query_param("region", "PL");
            then.status(200).json_body(json!({
                "results": [
                    tmdb_search_result(1, "Tajny agent", "The Secret Agent", "2025-04-04", "en", 6.2, 120, "Old spy thriller."),
                    tmdb_search_result(2, "Tajny agent", "The Amateur", "2026-04-10", "en", 7.8, 80, "A modern spy thriller."),
                ]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/search/movie")
                .query_param("query", "Tajny agent")
                .query_param("primary_release_year", "2026");
            then.status(200).json_body(json!({
                "results": [
                    tmdb_search_result(1, "Tajny agent", "The Secret Agent", "2025-04-04", "en", 6.2, 120, "Old spy thriller."),
                    tmdb_search_result(2, "Tajny agent", "The Amateur", "2026-04-10", "en", 7.8, 80, "A modern spy thriller."),
                ]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/1");
            then.status(200).json_body(tmdb_movie_details(
                1,
                "Tajny agent",
                "The Secret Agent",
                "2025-04-04",
                "en",
                98,
                6.2,
                120,
                "Old spy thriller.",
                &["Comedy"],
                &[("GB", "United Kingdom")],
                &["Someone Else"],
                &["Actor One"],
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/2");
            then.status(200).json_body(tmdb_movie_details(
                2,
                "Tajny agent",
                "The Amateur",
                "2026-04-10",
                "en",
                123,
                7.8,
                80,
                "A modern spy thriller.",
                &["Thriller"],
                &[("US", "United States")],
                &["James Hawes"],
                &["Rami Malek"],
            ));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Tajny agent",
                MovieLookupMetadata {
                    runtime_minutes: Some(123),
                    original_language_code: Some("EN".to_string()),
                    genre_tags: vec!["thriller".to_string()],
                    production_year: Some(2026),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "Tajny agent".to_string(),
            TmdbMovieDetails {
                rating: "7.8/10\n(głosy: 80)".to_string(),
                summary: "A modern spy thriller.".to_string(),
            }
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_strips_screening_suffixes_and_includes_adult_results() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(
            200,
            "OK",
            json!({
                "results": [tmdb_search_result(
                    3,
                    "Projekt Hail Mary",
                    "Project Hail Mary",
                    "2026-03-20",
                    "en",
                    8.1,
                    9000,
                    "A space survival story."
                )]
            }),
            &[],
        ),
        json_response(
            200,
            "OK",
            tmdb_movie_details(
                3,
                "Projekt Hail Mary",
                "Project Hail Mary",
                "2026-03-20",
                "en",
                146,
                8.1,
                9000,
                "A space survival story.",
                &["Science Fiction"],
                &[("US", "United States")],
                &["Phil Lord"],
                &["Ryan Gosling"],
            ),
            &[],
        ),
    ]);

    let client = ReqwestTmdbClient::with_base_urls(
        format!("{base_url}/authentication"),
        format!("{base_url}/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Projekt Hail Mary ukraiński dubbing",
                MovieLookupMetadata {
                    runtime_minutes: Some(146),
                    original_language_code: Some("EN".to_string()),
                    production_year: Some(2026),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    server.join().expect("test server should finish cleanly");

    assert_eq!(
        details,
        HashMap::from([(
            "Projekt Hail Mary ukraiński dubbing".to_string(),
            TmdbMovieDetails {
                rating: "8.1/10\n(głosy: 9000)".to_string(),
                summary: "A space survival story.".to_string(),
            }
        )])
    );

    let requests = recorded_requests.lock().expect("recorded requests lock poisoned");
    assert!(requests[0].contains("GET /search/movie?query=Projekt+Hail+Mary"));
    assert!(requests[0].contains("include_adult=true"));
    assert!(requests[0].contains("language=pl-PL"));
    assert!(requests[0].contains("region=PL"));
    assert!(!requests[0].contains("ukrai%C5%84ski+dubbing"));
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_uses_movie_page_fallback_metadata_to_break_ties() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "Amator");
            then.status(200).json_body(json!({
                "results": [
                    tmdb_search_result(11, "Amator", "Another Amateur", "2026-04-10", "en", 6.4, 300, "Wrong movie."),
                    tmdb_search_result(12, "Amator", "The Amateur", "2026-04-10", "en", 7.7, 1400, "Correct movie."),
                ]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "The Amateur");
            then.status(200).json_body(json!({
                "results": [tmdb_search_result(12, "Amator", "The Amateur", "2026-04-10", "en", 7.7, 1400, "Correct movie.")]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/11");
            then.status(200).json_body(tmdb_movie_details(
                11,
                "Amator",
                "Another Amateur",
                "2026-04-10",
                "en",
                109,
                6.4,
                300,
                "Wrong movie.",
                &["Drama"],
                &[("GB", "United Kingdom")],
                &["Wrong Director"],
                &["Wrong Actor"],
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/12");
            then.status(200).json_body(tmdb_movie_details(
                12,
                "Amator",
                "The Amateur",
                "2026-04-10",
                "en",
                123,
                7.7,
                1400,
                "Correct movie.",
                &["Thriller"],
                &[("US", "United States")],
                &["James Hawes"],
                &["Rami Malek", "Rachel Brosnahan"],
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/cinema-city/amator")
                .header("accept-language", "pl-PL,pl;q=0.9,en-US;q=0.8,en;q=0.7");
            then.status(200).body(
                r#"<html><body><script>
                    var filmDetails = {
                      "originalName": "The Amateur",
                      "releaseCountry": "USA",
                      "cast": "Rami Malek, Rachel Brosnahan",
                      "directors": "James Hawes",
                      "synopsis": "Cinema City synopsis."
                    };
                </script></body></html>"#,
            );
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Amator",
                MovieLookupMetadata {
                    movie_page_url: Some(server.url("/cinema-city/amator")),
                    runtime_minutes: Some(123),
                    genre_tags: vec!["thriller".to_string()],
                    original_language_code: Some("EN".to_string()),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "Amator".to_string(),
            TmdbMovieDetails {
                rating: "7.7/10\n(głosy: 1400)".to_string(),
                summary: "Correct movie.".to_string(),
            }
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_accepts_single_exact_title_match_with_partial_metadata() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "Krzyk 7");
            then.status(200).json_body(json!({
                "results": [tmdb_search_result(
                    61,
                    "Krzyk 7",
                    "Scream 7",
                    "2026-03-01",
                    "en",
                    6.5,
                    12,
                    "Ghostface returns."
                )]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/61");
            then.status(200).json_body(tmdb_movie_details(
                61,
                "Krzyk 7",
                "Scream 7",
                "2026-03-01",
                "en",
                115,
                6.5,
                12,
                "Ghostface returns.",
                &["Horror"],
                &[("US", "United States")],
                &["Kevin Williamson"],
                &["Neve Campbell"],
            ));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Krzyk 7",
                MovieLookupMetadata {
                    runtime_minutes: Some(115),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "Krzyk 7".to_string(),
            TmdbMovieDetails {
                rating: "6.5/10\n(głosy: 12)".to_string(),
                summary: "Ghostface returns.".to_string(),
            }
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_uses_alternate_titles_for_translated_searches() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "Ostatnia wieczerza");
            then.status(200).json_body(json!({
                "results": []
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "The Last Supper");
            then.status(200).json_body(json!({
                "results": [tmdb_search_result(
                    62,
                    "The Last Supper",
                    "The Last Supper",
                    "2026-03-20",
                    "en",
                    6.8,
                    84,
                    "Correct movie."
                )]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/62");
            then.status(200).json_body(tmdb_movie_details(
                62,
                "The Last Supper",
                "The Last Supper",
                "2026-03-20",
                "en",
                94,
                6.8,
                84,
                "Correct movie.",
                &["Drama"],
                &[("US", "United States")],
                &["Mauro Borrelli"],
                &["Jamie Ward"],
            ));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Ostatnia wieczerza",
                MovieLookupMetadata {
                    alternate_titles: vec!["The Last Supper".to_string()],
                    runtime_minutes: Some(94),
                    original_language_code: Some("EN".to_string()),
                    production_year: Some(2025),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "Ostatnia wieczerza".to_string(),
            TmdbMovieDetails {
                rating: "6.8/10\n(głosy: 84)".to_string(),
                summary: "Correct movie.".to_string(),
            }
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_strips_screening_suffixes_from_alternate_titles() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(200, "OK", json!({"results": []}), &[]),
        json_response(
            200,
            "OK",
            json!({
                "results": [tmdb_search_result(
                    63,
                    "Project Hail Mary",
                    "Project Hail Mary",
                    "2026-03-20",
                    "en",
                    7.9,
                    512,
                    "Ryland Grace wakes up alone in space."
                )]
            }),
            &[],
        ),
        json_response(
            200,
            "OK",
            tmdb_movie_details(
                63,
                "Project Hail Mary",
                "Project Hail Mary",
                "2026-03-20",
                "en",
                156,
                7.9,
                512,
                "Ryland Grace wakes up alone in space.",
                &["Science Fiction", "Drama", "Thriller"],
                &[("US", "United States")],
                &["Phil Lord"],
                &["Ryan Gosling"],
            ),
            &[],
        ),
    ]);

    let client = ReqwestTmdbClient::with_base_urls(
        format!("{base_url}/authentication"),
        format!("{base_url}/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "Projekt Hail Mary ukraiński dubbing",
                MovieLookupMetadata {
                    alternate_titles: vec!["Project Hail Mary Ukrainian dubbing".to_string()],
                    runtime_minutes: Some(156),
                    production_year: Some(2026),
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    server.join().expect("test server should finish cleanly");

    assert_eq!(
        details,
        HashMap::from([(
            "Projekt Hail Mary ukraiński dubbing".to_string(),
            TmdbMovieDetails {
                rating: "7.9/10\n(głosy: 512)".to_string(),
                summary: "Ryland Grace wakes up alone in space.".to_string(),
            }
        )])
    );

    let requests = recorded_requests.lock().expect("recorded requests lock poisoned");
    assert!(requests[0].contains("GET /search/movie?query=Projekt+Hail+Mary"));
    assert!(requests[1].contains("GET /search/movie?query=Project+Hail+Mary"));
    assert!(!requests[1].contains("Project+Hail+Mary+Ukrainian+dubbing"));
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_accepts_exact_title_match_when_details_break_tie() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "David");
            then.status(200).json_body(json!({
                "results": [
                    tmdb_search_result(71, "David", "David", "1997-03-23", "en", 7.2, 500, "Wrong David."),
                    tmdb_search_result(72, "David", "David", "2026-03-27", "en", 6.4, 3, "Correct David.")
                ]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/71");
            then.status(200).json_body(tmdb_movie_details(
                71,
                "David",
                "David",
                "1997-03-23",
                "en",
                96,
                7.2,
                500,
                "Wrong David.",
                &["Drama"],
                &[("US", "United States")],
                &["Wrong Director"],
                &["Wrong Actor"],
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/72");
            then.status(200).json_body(tmdb_movie_details(
                72,
                "David",
                "David",
                "2026-03-27",
                "en",
                109,
                6.4,
                3,
                "Correct David.",
                &["Animation", "Adventure", "Family"],
                &[("US", "United States")],
                &["Phil Cunningham"],
                &["Manny Perez"],
            ));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie_with_metadata(
                "David",
                MovieLookupMetadata {
                    runtime_minutes: Some(109),
                    genre_tags: vec![
                        "animowany".to_string(),
                        "przygodowy".to_string(),
                        "familijny".to_string(),
                    ],
                    ..MovieLookupMetadata::default()
                },
            )],
            "token",
        )
        .await
        .unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "David".to_string(),
            TmdbMovieDetails {
                rating: "6.4/10\n(głosy: 3)".to_string(),
                summary: "Correct David.".to_string(),
            }
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_leaves_low_confidence_matches_blank() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "Amator");
            then.status(200).json_body(json!({
                "results": [tmdb_search_result(21, "Amatorzy", "The Amateurs", "2024-04-10", "en", 6.0, 100, "Not the same movie.")]
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/movie/21");
            then.status(200).json_body(tmdb_movie_details(
                21,
                "Amatorzy",
                "The Amateurs",
                "2024-04-10",
                "en",
                101,
                6.0,
                100,
                "Not the same movie.",
                &["Comedy"],
                &[("GB", "United Kingdom")],
                &["Wrong Director"],
                &["Wrong Actor"],
            ));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details =
        client.get_movie_ratings_and_summaries(&[lookup_movie("Amator")], "token").await.unwrap();

    assert_eq!(
        details,
        HashMap::from([(
            "Amator".to_string(),
            TmdbMovieDetails { rating: String::new(), summary: String::new() },
        )])
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_accepts_legacy_v3_api_keys() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(
            200,
            "OK",
            json!({
                "results": [tmdb_search_result(
                    31,
                    "Garfield",
                    "The Garfield Movie",
                    "2024-05-24",
                    "en",
                    6.717,
                    184,
                    "Garfield jest najbardziej znanym kotem na świecie."
                )]
            }),
            &[],
        ),
        json_response(
            200,
            "OK",
            tmdb_movie_details(
                31,
                "Garfield",
                "The Garfield Movie",
                "2024-05-24",
                "en",
                101,
                6.717,
                184,
                "Garfield jest najbardziej znanym kotem na świecie.",
                &["Animation"],
                &[("US", "United States")],
                &["Mark Dindal"],
                &["Chris Pratt"],
            ),
            &[],
        ),
    ]);

    let client = ReqwestTmdbClient::with_base_urls(
        format!("{base_url}/authentication"),
        format!("{base_url}/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &[lookup_movie("Garfield")],
            "1234567890abcdef1234567890abcdef",
        )
        .await
        .unwrap();

    server.join().expect("test server should finish cleanly");

    assert_eq!(
        details,
        HashMap::from([(
            "Garfield".to_string(),
            TmdbMovieDetails {
                rating: "6.717/10\n(głosy: 184)".to_string(),
                summary: "Garfield jest najbardziej znanym kotem na świecie.".to_string(),
            }
        )])
    );

    let requests = recorded_requests.lock().expect("recorded requests lock poisoned");
    assert!(
        requests.iter().any(|request| request.contains("api_key=1234567890abcdef1234567890abcdef"))
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_retries_retryable_tmdb_responses() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(
            429,
            "Too Many Requests",
            json!({"status_message": "slow down"}),
            &[("Retry-After", "0")],
        ),
        json_response(
            200,
            "OK",
            json!({
                "results": [tmdb_search_result(
                    41,
                    "Garfield",
                    "The Garfield Movie",
                    "2024-05-24",
                    "en",
                    6.717,
                    184,
                    "Garfield jest najbardziej znanym kotem na świecie."
                )]
            }),
            &[],
        ),
        json_response(
            200,
            "OK",
            tmdb_movie_details(
                41,
                "Garfield",
                "The Garfield Movie",
                "2024-05-24",
                "en",
                101,
                6.717,
                184,
                "Garfield jest najbardziej znanym kotem na świecie.",
                &["Animation"],
                &[("US", "United States")],
                &["Mark Dindal"],
                &["Chris Pratt"],
            ),
            &[],
        ),
    ]);

    let client = ReqwestTmdbClient::with_base_urls(
        format!("{base_url}/authentication"),
        format!("{base_url}/search/movie"),
    )
    .unwrap()
    .with_retry_policy(RetryPolicy::new(2, Duration::ZERO, Duration::ZERO));

    let details =
        client.get_movie_ratings_and_summaries(&[lookup_movie("Garfield")], "token").await.unwrap();

    server.join().expect("test server should finish cleanly");

    assert_eq!(
        details,
        HashMap::from([(
            "Garfield".to_string(),
            TmdbMovieDetails {
                rating: "6.717/10\n(głosy: 184)".to_string(),
                summary: "Garfield jest najbardziej znanym kotem na świecie.".to_string(),
            }
        )])
    );

    let requests = recorded_requests.lock().expect("recorded requests lock poisoned");
    assert_eq!(requests.len(), 3);
    assert!(requests[0].contains("GET /search/movie?query=Garfield"));
    assert!(requests[1].contains("GET /search/movie?query=Garfield"));
    assert!(requests[2].contains("GET /movie/41?language=pl-PL&append_to_response=credits"));
}

#[tokio::test]
async fn verify_api_key_retries_retryable_tmdb_responses() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(
            500,
            "Internal Server Error",
            json!({"status_message": "temporary failure"}),
            &[],
        ),
        json_response(200, "OK", json!({"success": true}), &[]),
    ]);

    let client = ReqwestTmdbClient::with_base_urls(
        format!("{base_url}/authentication"),
        format!("{base_url}/search/movie"),
    )
    .unwrap()
    .with_retry_policy(RetryPolicy::new(2, Duration::ZERO, Duration::ZERO));

    assert!(client.verify_api_key("token").await);

    server.join().expect("test server should finish cleanly");

    let requests = recorded_requests.lock().expect("recorded requests lock poisoned");
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.contains("GET /authentication")));
    assert!(
        requests
            .iter()
            .all(|request| request.to_ascii_lowercase().contains("authorization: bearer token"))
    );
}

#[tokio::test]
async fn get_movie_ratings_and_summaries_reports_invalid_tmdb_json() {
    let server = MockServer::start_async().await;
    let invalid_json_mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/search/movie").query_param("query", "Garfield");
            then.status(200).header("content-type", "application/json").body(r#"{"results":["#);
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let error = client
        .get_movie_ratings_and_summaries(&[lookup_movie("Garfield")], "token")
        .await
        .expect_err("invalid TMDB JSON should surface as an error");

    invalid_json_mock.assert_async().await;
    assert!(matches!(
        error,
        AppError::Http(message)
            if message.contains("invalid JSON") && message.contains("Garfield")
    ));
}

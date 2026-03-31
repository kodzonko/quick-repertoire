mod support;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use httpmock::Method::GET;
use httpmock::MockServer;
use quick_repertoire::domain::TmdbMovieDetails;
use quick_repertoire::error::AppError;
use quick_repertoire::retry::RetryPolicy;
use quick_repertoire::tmdb::{
    ReqwestTmdbClient, TmdbService, ensure_single_result, parse_movie_rating, parse_movie_summary,
};
use serde_json::json;

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

#[tokio::test]
async fn get_movie_ratings_and_summaries_accepts_legacy_v3_api_keys() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/search/movie")
                .query_param("query", "Garfield")
                .query_param("api_key", "1234567890abcdef1234567890abcdef");
            then.status(200).json_body(json!({
                "results": [{
                    "vote_average": 6.717,
                    "vote_count": 184,
                    "overview": "Garfield jest najbardziej znanym kotem na świecie."
                }]
            }));
        })
        .await;

    let client = ReqwestTmdbClient::with_base_urls(
        server.url("/authentication"),
        server.url("/search/movie"),
    )
    .unwrap();

    let details = client
        .get_movie_ratings_and_summaries(
            &["Garfield".to_string()],
            "1234567890abcdef1234567890abcdef",
        )
        .await
        .unwrap();

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
                "results": [{
                    "vote_average": 6.717,
                    "vote_count": 184,
                    "overview": "Garfield jest najbardziej znanym kotem na świecie."
                }]
            }),
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
        client.get_movie_ratings_and_summaries(&["Garfield".to_string()], "token").await.unwrap();

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
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.contains("GET /search/movie?query=Garfield")));
    assert!(
        requests
            .iter()
            .all(|request| request.to_ascii_lowercase().contains("authorization: bearer token"))
    );
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
        .get_movie_ratings_and_summaries(&["Garfield".to_string()], "token")
        .await
        .expect_err("invalid TMDB JSON should surface as an error");

    invalid_json_mock.assert_async().await;
    assert!(matches!(
        error,
        AppError::Http(message)
            if message.contains("invalid JSON") && message.contains("Garfield")
    ));
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

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use httpmock::Method::GET;
use httpmock::MockServer;
use quick_repertoire::cinema::multikino::Multikino;
use quick_repertoire::cinema::registry::CinemaChainClient;
use quick_repertoire::domain::CinemaVenue;
use quick_repertoire::retry::RetryPolicy;
use serde_json::json;

fn build_client(server: &MockServer) -> Multikino {
    Multikino::new(server.base_url())
        .with_showings_api_base_url(format!("{}/api/microservice/showings", server.base_url()))
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
                .push(String::from_utf8_lossy(&request).into_owned());
            stream.write_all(response.as_bytes()).expect("response should be writable");
            stream.flush().expect("response should be flushed");
        }
    });

    (format!("http://{address}"), recorded_requests, server)
}

#[tokio::test]
async fn fetch_venues_parses_multikino_api_response() {
    let server = MockServer::start();
    let venues_mock = server.mock(|when, then| {
        when.method(GET).path("/api/microservice/showings/cinemas");
        then.status(200).json_body(json!({
            "result": [
                {
                    "alpha": "W",
                    "cinemas": [
                        {
                            "cinemaId": "0034",
                            "cinemaName": "Warszawa Złote Tarasy",
                            "fullName": "Warszawa Złote Tarasy"
                        },
                        {
                            "cinemaId": "0052",
                            "cinemaName": "Wrocław Pasaż Grunwaldzki",
                            "fullName": "Wrocław Pasaż Grunwaldzki"
                        }
                    ]
                },
                {
                    "alpha": "Ł",
                    "cinemas": [
                        {
                            "cinemaId": "0010",
                            "cinemaName": "Łódź",
                            "fullName": "Łódź"
                        }
                    ]
                }
            ],
            "responseCode": 0,
            "errorMessage": null
        }));
    });

    let client = build_client(&server);

    let venues = client.fetch_venues().await.unwrap();

    venues_mock.assert();
    let mut actual = venues
        .into_iter()
        .map(|venue| (venue.chain_id, venue.venue_id, venue.venue_name))
        .collect::<Vec<_>>();
    actual.sort_by(|left, right| left.1.cmp(&right.1));

    assert_eq!(
        actual,
        vec![
            ("multikino".to_string(), "0010".to_string(), "Łódź".to_string(),),
            ("multikino".to_string(), "0034".to_string(), "Warszawa Złote Tarasy".to_string(),),
            ("multikino".to_string(), "0052".to_string(), "Wrocław Pasaż Grunwaldzki".to_string(),),
        ]
    );
}

#[tokio::test]
async fn fetch_venues_trims_ids_and_deduplicates_whitespace_variants() {
    let server = MockServer::start();
    let venues_mock = server.mock(|when, then| {
        when.method(GET).path("/api/microservice/showings/cinemas");
        then.status(200).json_body(json!({
            "result": [
                {
                    "alpha": "W",
                    "cinemas": [
                        {
                            "cinemaId": " 0034 ",
                            "cinemaName": "Warszawa Złote Tarasy",
                            "fullName": "  Warszawa Złote Tarasy  "
                        },
                        {
                            "cinemaId": "0034",
                            "cinemaName": "Warszawa duplicate",
                            "fullName": "Warszawa duplicate"
                        },
                        {
                            "cinemaId": " 0052",
                            "cinemaName": "Wrocław Pasaż Grunwaldzki",
                            "fullName": "Wrocław Pasaż Grunwaldzki"
                        },
                        {
                            "cinemaId": "   ",
                            "cinemaName": "Ignored",
                            "fullName": "Ignored"
                        }
                    ]
                }
            ],
            "responseCode": 0,
            "errorMessage": null
        }));
    });

    let client = build_client(&server);

    let venues = client.fetch_venues().await.unwrap();

    venues_mock.assert();
    let mut actual =
        venues.into_iter().map(|venue| (venue.venue_id, venue.venue_name)).collect::<Vec<_>>();
    actual.sort_by(|left, right| left.0.cmp(&right.0));

    assert_eq!(
        actual,
        vec![
            ("0034".to_string(), "Warszawa Złote Tarasy".to_string()),
            ("0052".to_string(), "Wrocław Pasaż Grunwaldzki".to_string()),
        ]
    );
}

#[tokio::test]
async fn fetch_venues_honors_retry_after_header_on_rate_limit() {
    let (base_url, recorded_requests, server) = start_sequenced_http_server(vec![
        json_response(
            429,
            "Too Many Requests",
            json!({"message": "slow down"}),
            &[("Retry-After", "0")],
        ),
        json_response(
            200,
            "OK",
            json!({
                "result": [
                    {
                        "alpha": "W",
                        "cinemas": [
                            {
                                "cinemaId": "0034",
                                "cinemaName": "Warszawa Złote Tarasy",
                                "fullName": "Warszawa Złote Tarasy"
                            }
                        ]
                    }
                ],
                "responseCode": 0,
                "errorMessage": null
            }),
            &[],
        ),
    ]);

    let client = Multikino::new(base_url.clone())
        .with_showings_api_base_url(format!("{base_url}/api/microservice/showings"))
        .with_retry_policy(RetryPolicy::new(2, Duration::from_secs(1), Duration::from_secs(1)));

    let started_at = Instant::now();
    let venues = client.fetch_venues().await.unwrap();
    let elapsed = started_at.elapsed();

    server.join().expect("test server should finish cleanly");

    assert!(
        elapsed < Duration::from_millis(500),
        "expected Retry-After header to avoid generic backoff, elapsed was {elapsed:?}"
    );
    assert_eq!(recorded_requests.lock().expect("recorded requests lock poisoned").len(), 2);
    assert_eq!(venues.len(), 1);
    assert_eq!(venues[0].venue_id, "0034");
}

#[tokio::test]
async fn fetch_repertoire_groups_multikino_sessions_by_format_and_language() {
    let server = MockServer::start();
    let films_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/microservice/showings/cinemas/0034/films")
            .query_param("minEmbargoLevel", "3")
            .query_param("includesSession", "true")
            .query_param("includeSessionAttributes", "true");
        then.status(200).json_body(json!({
            "result": [
                {
                    "filmId": "HO00002328",
                    "filmUrl": format!("{}/filmy/projekt-hail-mary", server.base_url()),
                    "filmTitle": "Projekt Hail Mary",
                    "originalTitle": "Project Hail Mary",
                    "releaseDate": "2026-03-20T00:00:00",
                    "runningTime": 157,
                    "isDurationUnknown": false,
                    "genres": ["science fiction", "thriller"],
                    "showingGroups": [
                        {
                            "date": "2026-04-03T00:00:00",
                            "sessions": [
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00002328/64811",
                                    "startTime": "2026-04-03T18:30:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "NAPISY", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"},
                                        {"name": "Single Seat", "attributeType": "Session"}
                                    ]
                                },
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00002328/64812",
                                    "startTime": "2026-04-03T20:00:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "NAPISY", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"},
                                        {"name": "SUPERHIT", "attributeType": "Session"}
                                    ]
                                },
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00002328/64813",
                                    "startTime": "2026-04-03T21:15:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "DUBBING", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"}
                                    ]
                                }
                            ]
                        },
                        {
                            "date": "2026-04-04T00:00:00",
                            "sessions": [
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00002328/64814",
                                    "startTime": "2026-04-04T10:30:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "NAPISY", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"}
                                    ]
                                }
                            ]
                        }
                    ]
                },
                {
                    "filmId": "HO00002255",
                    "filmUrl": format!(
                        "{}/filmy/rbo-sezon-kinowy-2025-26-czarodziejski-flet",
                        server.base_url()
                    ),
                    "filmTitle": "Royal Ballet and Opera Sezon Kinowy 2025-26: Czarodziejski flet",
                    "originalTitle": "RBO Cinema Season 2025-26: The Magic Flute",
                    "releaseDate": "2026-04-26T00:00:00",
                    "runningTime": 210,
                    "isDurationUnknown": false,
                    "genres": ["opera"],
                    "showingGroups": [
                        {
                            "date": "2026-04-03T00:00:00",
                            "sessions": [
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00002255/46519",
                                    "startTime": "2026-04-03T15:00:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "NAPISY", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"},
                                        {"name": "KULTOWE KINO", "attributeType": "Session"},
                                        {"name": "OPERA", "attributeType": "Movie"}
                                    ]
                                }
                            ]
                        }
                    ]
                },
                {
                    "filmId": "HO00009999",
                    "filmUrl": format!("{}/filmy/inna-data", server.base_url()),
                    "filmTitle": "Inna data",
                    "runningTime": 101,
                    "isDurationUnknown": false,
                    "genres": ["dramat"],
                    "showingGroups": [
                        {
                            "date": "2026-04-07T00:00:00",
                            "sessions": [
                                {
                                    "bookingUrl": "/rezerwacja-biletow/podsumowanie/0034/HO00009999/99999",
                                    "startTime": "2026-04-07T19:00:00",
                                    "isSoldOut": false,
                                    "isBookingAvailable": true,
                                    "attributes": [
                                        {"name": "NAPISY", "attributeType": "Language"},
                                        {"name": "2D", "attributeType": "Session"}
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ],
            "responseCode": 0,
            "errorMessage": null
        }));
    });
    let attribute_groups_mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/microservice/showings/attributes/showingAttributeGroups")
            .query_param("cinemaId", "0034")
            .query_param("minEmbargoLevel", "2");
        then.status(200).json_body(json!({
            "result": [
                {
                    "name": "Rodzaj pokazu",
                    "showingAttributes": [
                        {"name": "2D", "attributeType": "Session"},
                        {"name": "KULTOWE KINO", "attributeType": "Session"}
                    ]
                },
                {
                    "name": "Wersja językowa",
                    "showingAttributes": [
                        {"name": "DUBBING", "attributeType": "Language"},
                        {"name": "NAPISY", "attributeType": "Language"},
                        {"name": "POLSKI", "attributeType": "Language"}
                    ]
                }
            ],
            "responseCode": 0,
            "errorMessage": null
        }));
    });

    let client = build_client(&server);
    let venue = CinemaVenue {
        chain_id: "multikino".to_string(),
        venue_id: "0034".to_string(),
        venue_name: "Warszawa Złote Tarasy".to_string(),
    };

    let repertoire = client.fetch_repertoire("2026-04-03", &venue).await.unwrap();

    films_mock.assert();
    attribute_groups_mock.assert();
    assert_eq!(repertoire.len(), 2);

    let hail_mary_page_url = format!("{}/filmy/projekt-hail-mary", server.base_url());
    let hail_mary_first_booking =
        format!("{}/rezerwacja-biletow/podsumowanie/0034/HO00002328/64811", server.base_url());
    let hail_mary_second_booking =
        format!("{}/rezerwacja-biletow/podsumowanie/0034/HO00002328/64812", server.base_url());

    let hail_mary = &repertoire[0];
    assert_eq!(hail_mary.title, "Projekt Hail Mary");
    assert_eq!(hail_mary.genres, "science fiction, thriller");
    assert_eq!(hail_mary.play_length, "157 min");
    assert_eq!(hail_mary.original_language, "Brak danych");
    assert_eq!(hail_mary.lookup_metadata.chain_movie_id.as_deref(), Some("HO00002328"));
    assert_eq!(
        hail_mary.lookup_metadata.movie_page_url.as_deref(),
        Some(hail_mary_page_url.as_str())
    );
    assert_eq!(hail_mary.lookup_metadata.alternate_titles, vec!["Project Hail Mary".to_string()]);
    assert_eq!(hail_mary.lookup_metadata.runtime_minutes, Some(157));
    assert_eq!(hail_mary.lookup_metadata.polish_premiere_date.as_deref(), Some("2026-03-20"));
    assert_eq!(hail_mary.lookup_metadata.production_year, Some(2026));
    assert_eq!(hail_mary.lookup_metadata.genre_tags, vec!["science fiction", "thriller"]);
    assert_eq!(hail_mary.play_details.len(), 2);
    assert_eq!(hail_mary.play_details[0].format, "2D");
    assert_eq!(hail_mary.play_details[0].play_language, "NAPISY");
    assert_eq!(
        hail_mary.play_details[0]
            .play_times
            .iter()
            .map(|play_time| (play_time.value.as_str(), play_time.url.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("18:30", Some(hail_mary_first_booking.as_str())),
            ("20:00", Some(hail_mary_second_booking.as_str())),
        ]
    );
    assert_eq!(hail_mary.play_details[1].format, "2D");
    assert_eq!(hail_mary.play_details[1].play_language, "DUBBING");
    assert_eq!(hail_mary.play_details[1].play_times[0].value, "21:15");

    let magic_flute = &repertoire[1];
    assert_eq!(
        magic_flute.title,
        "Royal Ballet and Opera Sezon Kinowy 2025-26: Czarodziejski flet"
    );
    assert_eq!(magic_flute.genres, "opera");
    assert_eq!(magic_flute.play_details.len(), 1);
    assert_eq!(magic_flute.play_details[0].format, "2D KULTOWE KINO");
    assert_eq!(magic_flute.play_details[0].play_language, "NAPISY");
    assert_eq!(magic_flute.play_details[0].play_times[0].value, "15:00");
}

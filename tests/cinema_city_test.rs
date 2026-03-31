mod support;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use httpmock::Method::GET;
use httpmock::MockServer;
use quick_repertoire::cinema::cinema_city::CinemaCity;
use quick_repertoire::cinema::cinema_city::HtmlRenderer;
use quick_repertoire::cinema::registry::CinemaChainClient;
use quick_repertoire::domain::{CinemaVenue, MoviePlayTime};
use quick_repertoire::error::{AppError, AppResult};
use quick_repertoire::retry::RetryPolicy;

use support::FakeHtmlRenderer;

fn rendered_repertoire_html() -> String {
    r#"
    <div class="row qb-movie">
      <h3 class="qb-movie-name">65</h3>
      <div class="qb-movie-info-wrapper">
        <span aria-label="original-lang">EN</span>
      </div>
      <div class="qb-movie-info-column">
        <ul class="qb-screening-attributes">
          <li><span aria-label="Screening type">2D</span></li>
          <li><span aria-label="subAbbr">NAP</span></li>
          <li><span aria-label="subbed-lang">PL</span></li>
        </ul>
        <a class="btn btn-primary btn-lg">17:45</a>
        <a class="btn btn-primary btn-lg">19:50</a>
      </div>
    </div>
    <div class="row qb-movie">
      <h3 class="qb-movie-name">John Wick 4</h3>
      <div class="qb-movie-info-wrapper">
        <span>| Action | Thriller |</span>
        <span>169 min</span>
        <span aria-label="original-lang">EN</span>
      </div>
      <div class="qb-movie-info-column">
        <h4>Przedsprzedaż</h4>
        <ul class="qb-screening-attributes">
          <li><span aria-label="Screening type">4DX</span></li>
        </ul>
        <a class="btn btn-primary btn-lg">21:00</a>
      </div>
    </div>
    <div class="row qb-movie">
      <h3 class="qb-movie-name">Dungeons & Dragons</h3>
      <div class="qb-movie-info-wrapper">
        <span>| Fantasy, Adventure |</span>
        <span>134 min</span>
        <span aria-label="original-lang">EN</span>
      </div>
      <div class="qb-movie-info-column">
        <ul class="qb-screening-attributes">
          <li><span aria-label="Screening type">IMAX</span></li>
          <li><span aria-label="Screening type">3D</span></li>
          <li><span aria-label="noSubs">BEZ NAP</span></li>
        </ul>
        <a class="btn btn-primary btn-lg">20:15</a>
      </div>
    </div>
    "#
    .to_string()
}

fn rendered_repertoire_html_with_current_language_markup() -> String {
    r#"
    <h2 class="mr-sm">Repertuar Cinema City Wroclaw - Wroclavia</h2>
    <div class="row qb-movie">
      <a class="qb-movie-link">
        <h3 class="qb-movie-name">Oni cię zabiją</h3>
      </a>
      <div class="qb-movie-info-wrapper">
        <div class="pt-xs">
          <span class="mr-sm">Horror <span class="ml-xs">|</span></span>
          <span class="mr-xs">94 min</span>
        </div>
      </div>
      <div class="events col-xs-12">
        <div class="type-row">
          <div class="qb-movie-info-column">
            <ul class="qb-screening-attributes">
              <li><span aria-label="Screening type: 2D">2D</span></li>
              <li><span aria-label="Screening type: Projekcja Laserowa BARCO">Projekcja Laserowa BARCO</span></li>
            </ul>
            <a class="btn btn-primary btn-lg">21:40</a>
            <ul class="qb-movie-attributes">
              <li><span aria-label="original-lang-en">EN</span></li>
              <li><span aria-label="subAbbr">FILM Z NAPISAMI</span></li>
              <li><span aria-label="first-subbed-lang-pl">PL</span></li>
            </ul>
          </div>
        </div>
      </div>
    </div>
    "#
    .to_string()
}

#[derive(Default)]
struct RecordingHtmlRenderer {
    urls: Mutex<VecDeque<String>>,
    html: String,
}

#[async_trait]
impl HtmlRenderer for RecordingHtmlRenderer {
    async fn render(&self, url: &str, _wait_selector: &str) -> AppResult<String> {
        self.urls.lock().expect("rendered url list lock poisoned").push_back(url.to_string());
        Ok(self.html.clone())
    }
}

struct SequencedHtmlRenderer {
    responses: Mutex<VecDeque<AppResult<String>>>,
    call_count: Mutex<usize>,
}

impl SequencedHtmlRenderer {
    fn new(responses: Vec<AppResult<String>>) -> Self {
        Self { responses: Mutex::new(VecDeque::from(responses)), call_count: Mutex::new(0) }
    }

    fn call_count(&self) -> usize {
        *self.call_count.lock().expect("render call count lock poisoned")
    }
}

#[async_trait]
impl HtmlRenderer for SequencedHtmlRenderer {
    async fn render(&self, _url: &str, _wait_selector: &str) -> AppResult<String> {
        *self.call_count.lock().expect("render call count lock poisoned") += 1;
        self.responses
            .lock()
            .expect("render responses lock poisoned")
            .pop_front()
            .expect("render response must be configured")
    }
}

#[tokio::test]
async fn fetch_repertoire_parses_inline_html_fixture_and_skips_presales() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: rendered_repertoire_html(),
        }),
    )
    .with_quickbook_api_base_url("");
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1080".to_string(),
        venue_name: "Lodz - Manufaktura".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2023-04-01", &venue_data).await.unwrap();

    assert_eq!(repertoire.len(), 2);
    assert_eq!(repertoire[0].title, "65");
    assert_eq!(repertoire[0].genres, "Brak danych");
    assert_eq!(repertoire[0].play_length, "Brak danych");
    assert_eq!(repertoire[0].original_language, "EN");
    assert_eq!(repertoire[0].play_details[0].format, "2D");
    assert_eq!(repertoire[0].play_details[0].play_language, "NAP: PL");
    assert_eq!(
        repertoire[0].play_details[0].play_times,
        vec![
            MoviePlayTime { value: "17:45".to_string(), url: None },
            MoviePlayTime { value: "19:50".to_string(), url: None },
        ]
    );
    assert_eq!(repertoire[1].title, "Dungeons & Dragons");
    assert_eq!(repertoire[1].genres, "Fantasy, Adventure");
    assert_eq!(repertoire[1].play_length, "134 min");
    assert_eq!(repertoire[1].original_language, "EN");
    assert_eq!(repertoire[1].play_details[0].format, "IMAX 3D");
    assert_eq!(repertoire[1].play_details[0].play_language, "BEZ NAP");
    assert_eq!(
        repertoire[1].play_details[0].play_times,
        vec![MoviePlayTime { value: "20:15".to_string(), url: None }]
    );
}

#[tokio::test]
async fn fetch_repertoire_upgrades_legacy_url_template_to_canonical_cinema_route() {
    let renderer = Arc::new(RecordingHtmlRenderer {
        urls: Mutex::new(VecDeque::new()),
        html: rendered_repertoire_html(),
    });
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        renderer.clone(),
    )
    .with_quickbook_api_base_url("");
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1097".to_string(),
        venue_name: "Wroclaw - Wroclavia".to_string(),
    };

    let _ = cinema.fetch_repertoire("2026-03-31", &venue_data).await.unwrap();

    let rendered_url = renderer
        .urls
        .lock()
        .expect("rendered url list lock poisoned")
        .front()
        .cloned()
        .expect("renderer should record a single repertoire url");
    assert_eq!(
        rendered_url,
        "https://www.cinema-city.pl/kina/wroclavia/1097#/buy-tickets-by-cinema?in-cinema=1097&at=2026-03-31&view-mode=list"
    );
}

#[tokio::test]
async fn fetch_repertoire_parses_current_language_markup_from_live_schedule_page() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/kina/{cinema_venue_slug}/{cinema_venue_id}#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}&view-mode=list".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: rendered_repertoire_html_with_current_language_markup(),
        }),
    )
    .with_quickbook_api_base_url("");
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1097".to_string(),
        venue_name: "Wroclaw - Wroclavia".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2026-03-31", &venue_data).await.unwrap();

    assert_eq!(repertoire.len(), 1);
    assert_eq!(repertoire[0].title, "Oni cię zabiją");
    assert_eq!(repertoire[0].genres, "Horror");
    assert_eq!(repertoire[0].play_length, "94 min");
    assert_eq!(repertoire[0].original_language, "EN");
    assert_eq!(
        repertoire[0].play_details,
        vec![quick_repertoire::domain::MoviePlayDetails {
            format: "2D Projekcja Laserowa BARCO".to_string(),
            play_language: "FILM Z NAPISAMI: PL".to_string(),
            play_times: vec![MoviePlayTime { value: "21:40".to_string(), url: None }],
        }]
    );
}

#[tokio::test]
async fn fetch_repertoire_adds_movie_page_links_for_bookable_showtimes() {
    let server = MockServer::start_async().await;
    let film_events_mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/pl/data-api-service/v1/quickbook/10103/film-events/in-cinema/1097/at-date/2026-03-31");
            then.status(200).header("content-type", "application/json").body(
                r#"{
                  "body": {
                    "films": [
                      {
                        "id": "7945s2r",
                        "name": "Oni cię zabiją",
                        "link": "https://www.cinema-city.pl/filmy/oni-cie-zabija/7945s2r"
                      }
                    ],
                    "events": [
                      {
                        "filmId": "7945s2r",
                        "eventDateTime": "2026-03-31T21:40:00",
                        "bookingLink": "https://tickets.cinema-city.pl/api/order/1350898?lang=pl",
                        "soldOut": false,
                        "compositeBookingLink": {
                          "blockOnlineSales": false
                        }
                      }
                    ]
                  }
                }"#,
            );
        })
        .await;
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/kina/{cinema_venue_slug}/{cinema_venue_id}#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}&view-mode=list".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: rendered_repertoire_html_with_current_language_markup(),
        }),
    )
    .with_quickbook_api_base_url(server.url("/pl/data-api-service"));
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1097".to_string(),
        venue_name: "Wroclaw - Wroclavia".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2026-03-31", &venue_data).await.unwrap();

    film_events_mock.assert_async().await;
    assert_eq!(
        repertoire[0].play_details[0].play_times,
        vec![MoviePlayTime {
            value: "21:40".to_string(),
            url: Some("https://www.cinema-city.pl/filmy/oni-cie-zabija/7945s2r".to_string()),
        }]
    );
}

#[tokio::test]
async fn fetch_repertoire_retries_transient_browser_failures() {
    let renderer = Arc::new(SequencedHtmlRenderer::new(vec![
        Err(AppError::BrowserUnavailable("temporary page navigation failure".to_string())),
        Ok(rendered_repertoire_html()),
    ]));
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        renderer.clone(),
    )
    .with_retry_policy(RetryPolicy::new(2, Duration::ZERO, Duration::ZERO))
    .with_quickbook_api_base_url("");
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1080".to_string(),
        venue_name: "Lodz - Manufaktura".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2023-04-01", &venue_data).await.unwrap();

    assert_eq!(repertoire.len(), 2);
    assert_eq!(renderer.call_count(), 2);
}

#[tokio::test]
async fn fetch_venues_filters_out_invalid_venues() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: r#"
            <select>
              <option value="">Wybierz kino</option>
              <option value="1080" data-tokens="Lodz - Manufaktura">Lodz - Manufaktura</option>
              <option value="1097" data-tokens="Wroclaw - Wroclavia">Wroclaw - Wroclavia</option>
              <option value="invalid" data-tokens="Ignored">Ignored</option>
              <option value="9999" data-tokens="null">Ignored</option>
            </select>
            "#
            .to_string(),
        }),
    );

    let venues = cinema.fetch_venues().await.unwrap();

    assert_eq!(
        venues
            .into_iter()
            .map(|venue| (venue.chain_id, venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![
            ("cinema-city".to_string(), "Lodz - Manufaktura".to_string(), "1080".to_string()),
            ("cinema-city".to_string(), "Wroclaw - Wroclavia".to_string(), "1097".to_string())
        ]
    );
}

#[tokio::test]
async fn fetch_venues_parses_embedded_api_sites_list_markup() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: r#"
            <html>
              <body>
                <script>
                  var tenantId = "10103",
                      apiSitesList = [
                        {"externalCode":"1080","name":"Łódź Manufaktura","address":{"city":"Łódź"}},
                        {"externalCode":"1097","name":"Wrocław - Wroclavia","address":{"city":"Wrocław"}},
                        {"externalCode":"1074","name":"Warszawa -  Arkadia","address":{"city":"Warszawa"}},
                        {"externalCode":"invalid","name":"Ignored","address":{"city":"Warszawa"}},
                        {"externalCode":"9999","name":"","address":{"city":"Warszawa"}}
                      ],
                      pluginLocale = "pl-pl";
                </script>
              </body>
            </html>
            "#
            .to_string(),
        }),
    );

    let venues = cinema.fetch_venues().await.unwrap();

    assert_eq!(
        venues
            .into_iter()
            .map(|venue| (venue.chain_id, venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![
            ("cinema-city".to_string(), "Lodz - Manufaktura".to_string(), "1080".to_string()),
            ("cinema-city".to_string(), "Wroclaw - Wroclavia".to_string(), "1097".to_string()),
            ("cinema-city".to_string(), "Warszawa - Arkadia".to_string(), "1074".to_string())
        ]
    );
}

#[tokio::test]
async fn fetch_venues_reports_invalid_embedded_api_sites_list_json() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: r#"
            <html>
              <body>
                <script>
                  var apiSitesList = [{"externalCode":"1080","name":];
                </script>
              </body>
            </html>
            "#
            .to_string(),
        }),
    );

    let error = cinema
        .fetch_venues()
        .await
        .expect_err("invalid embedded venues JSON should return an error");

    assert!(matches!(
        error,
        AppError::BrowserUnavailable(message)
            if message.contains("Nie udało się odczytać listy lokali Cinema City")
    ));
}

#[tokio::test]
async fn fetch_venues_retries_transient_browser_failures() {
    let renderer = Arc::new(SequencedHtmlRenderer::new(vec![
        Err(AppError::BrowserUnavailable("temporary venue page timeout".to_string())),
        Ok(r#"
            <select>
              <option value="1080" data-tokens="Lodz - Manufaktura">Lodz - Manufaktura</option>
            </select>
            "#
        .to_string()),
    ]));
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        renderer.clone(),
    )
    .with_retry_policy(RetryPolicy::new(2, Duration::ZERO, Duration::ZERO));

    let venues = cinema.fetch_venues().await.unwrap();

    assert_eq!(venues.len(), 1);
    assert_eq!(venues[0].venue_name, "Lodz - Manufaktura");
    assert_eq!(renderer.call_count(), 2);
}

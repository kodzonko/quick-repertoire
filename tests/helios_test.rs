mod support;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use quick_repertoire::cinema::helios::{
    DEFAULT_HELIOS_BASE_URL, DEFAULT_HELIOS_VENUES_URL, Helios,
};
use quick_repertoire::cinema::registry::CinemaChainClient;
use quick_repertoire::domain::{CinemaVenue, Repertoire};
use serde_json::json;
use support::{FakeHtmlRenderer, FakeRenderedPageRenderer};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/helios").join(name)
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("fixture must be readable")
}

fn lodz_venue() -> CinemaVenue {
    CinemaVenue {
        chain_id: "helios".to_string(),
        venue_id: "lodz/kino-helios".to_string(),
        venue_name: "Łódź - Helios".to_string(),
    }
}

fn build_live_client() -> Helios {
    let mut evaluations = HashMap::new();
    evaluations.insert("repertoire".to_string(), fixture("lodz-repertoire-state.json"));
    evaluations.insert("current_cinema".to_string(), fixture("lodz-current-cinema.json"));

    Helios::new(
        DEFAULT_HELIOS_BASE_URL,
        DEFAULT_HELIOS_VENUES_URL,
        Arc::new(FakeRenderedPageRenderer { html: fixture("lodz-repertoire.html"), evaluations }),
    )
}

fn find_movie<'a>(repertoire: &'a [Repertoire], title: &str) -> &'a Repertoire {
    repertoire
        .iter()
        .find(|movie| movie.title == title)
        .unwrap_or_else(|| panic!("movie `{title}` should be present"))
}

#[tokio::test]
async fn fetch_venues_parses_live_homepage_fixture() {
    let client = Helios::new(
        DEFAULT_HELIOS_BASE_URL,
        DEFAULT_HELIOS_VENUES_URL,
        Arc::new(FakeHtmlRenderer { html: fixture("home.html") }),
    );

    let venues = client.fetch_venues().await.unwrap();

    assert_eq!(venues.len(), 52);
    assert!(venues.iter().any(|venue| {
        venue.venue_id == "lodz/kino-helios" && venue.venue_name == "Łódź - Helios"
    }));
    assert!(venues.iter().any(|venue| {
        venue.venue_id == "gdynia/kino-helios" && venue.venue_name == "Gdynia - Helios"
    }));
}

#[tokio::test]
async fn fetch_repertoire_parses_live_state_for_multiple_dates_and_groups_showtimes() {
    let client = build_live_client();

    let today = client.fetch_repertoire("2026-04-01", &lodz_venue()).await.unwrap();
    let tomorrow = client.fetch_repertoire("2026-04-02", &lodz_venue()).await.unwrap();

    assert_eq!(today.len(), 20);
    assert_eq!(tomorrow.len(), 16);
    assert!(today.iter().any(|movie| movie.title == "Pojedynek - KNT"));
    assert!(!tomorrow.iter().any(|movie| movie.title == "Pojedynek - KNT"));

    let hopnieci = find_movie(&today, "Hopnięci");
    assert_eq!(hopnieci.genres, "animowany");
    assert_eq!(hopnieci.play_length, "106 min");
    assert_eq!(hopnieci.original_language, "Brak danych");
    assert_eq!(
        hopnieci
            .play_details
            .iter()
            .map(|detail| (detail.format.as_str(), detail.play_language.as_str()))
            .collect::<Vec<_>>(),
        vec![("Dream 2D Atmos", "Dubbing"), ("2D", "Dubbing")]
    );
    assert_eq!(
        hopnieci.play_details[0].play_times[0].url.as_deref(),
        Some(
            "https://bilety.helios.pl/screen/07583507-e1ad-48f6-94f6-8e4ade256a0e?cinemaId=46055d88-5f34-44a0-9584-b041caa71e26&backUrl=https%3A%2F%2Fhelios.pl%2Flodz%2Fkino-helios%2Frepertuar&item_id=f5d309a0-b1a8-4b8f-b6aa-82300425cc59&item_source_id=4172"
        )
    );
    assert_eq!(
        hopnieci.lookup_metadata.chain_movie_id.as_deref(),
        Some("f5d309a0-b1a8-4b8f-b6aa-82300425cc59")
    );
    assert_eq!(
        hopnieci.lookup_metadata.movie_page_url.as_deref(),
        Some("https://helios.pl/lodz/kino-helios/filmy/hopnieci-4172")
    );
    assert_eq!(hopnieci.lookup_metadata.alternate_titles, vec!["Hoppers".to_string()]);

    let hail_mary = find_movie(&today, "Projekt Hail Mary");
    assert_eq!(
        hail_mary
            .play_details
            .iter()
            .map(|detail| (detail.format.as_str(), detail.play_times.len()))
            .collect::<Vec<_>>(),
        vec![("2D", 1), ("2D Atmos", 3), ("Dream 2D Atmos", 2)]
    );
    assert_eq!(hail_mary.lookup_metadata.alternate_titles, vec!["Project Hail Mary".to_string()]);

    let knt = find_movie(&today, "Pojedynek - KNT");
    assert_eq!(knt.genres, "dramat, historyczny");
    assert_eq!(knt.play_length, "133 min");
    assert_eq!(knt.play_details.len(), 1);
    assert_eq!(knt.play_details[0].format, "2D");
    assert_eq!(knt.play_details[0].play_language, "Napisy");
    assert_eq!(
        knt.lookup_metadata.chain_movie_id.as_deref(),
        Some("e09a334e-42f5-4b8b-82c6-ed98db9b0402")
    );
    assert_eq!(
        knt.lookup_metadata.movie_page_url.as_deref(),
        Some("https://helios.pl/lodz/kino-helios/filmy/pojedynek-4220")
    );
    assert_eq!(knt.lookup_metadata.alternate_titles, vec!["Pojedynek".to_string()]);

    let tomorrow_hopnieci = find_movie(&tomorrow, "Hopnięci");
    assert_eq!(
        tomorrow_hopnieci
            .play_details
            .iter()
            .map(|detail| detail
                .play_times
                .iter()
                .map(|time| time.value.as_str())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec!["11:30"], vec!["14:00", "16:45"]]
    );
}

#[tokio::test]
async fn fetch_repertoire_handles_imax_and_missing_language_from_state() {
    let repertoire_state = json!({
        "list": [
            {
                "id": 9001,
                "sourceId": "movie-source",
                "title": "Imax Test",
                "titleOriginal": "Imax Test Original",
                "slug": "imax-test",
                "duration": 120,
                "genres": [{"name": "science fiction"}],
                "premiereDate": "2026-05-01",
                "cinemaPremiereDate": null,
                "isEvent": false,
                "isImax": true
            }
        ],
        "screenings": {
            "2026-05-01": {
                "m9001": {
                    "screenings": [
                        {
                            "timeFrom": "2026-05-01 19:30:00",
                            "sourceId": "screening-source",
                            "cinemaSourceId": "cinema-source",
                            "cinemaScreen": {"feature": "Imax"},
                            "moviePrint": {
                                "printType": "3D",
                                "printRelease": "IMAX/3D",
                                "soundType": "5.1",
                                "speakingTypeLabel": null
                            }
                        }
                    ]
                }
            }
        }
    });
    let current_cinema = json!({
        "slugCity": "warszawa",
        "slug": "kino-helios-blue-city"
    });
    let client = Helios::new(
        DEFAULT_HELIOS_BASE_URL,
        DEFAULT_HELIOS_VENUES_URL,
        Arc::new(FakeRenderedPageRenderer {
            html: fixture("lodz-repertoire.html"),
            evaluations: HashMap::from([
                ("repertoire".to_string(), serde_json::to_string(&repertoire_state).unwrap()),
                ("current_cinema".to_string(), serde_json::to_string(&current_cinema).unwrap()),
            ]),
        }),
    );
    let venue = CinemaVenue {
        chain_id: "helios".to_string(),
        venue_id: "warszawa/kino-helios-blue-city".to_string(),
        venue_name: "Warszawa - Helios Blue City".to_string(),
    };

    let repertoire = client.fetch_repertoire("2026-05-01", &venue).await.unwrap();

    assert_eq!(repertoire.len(), 1);
    assert_eq!(repertoire[0].play_details[0].format, "IMAX 3D");
    assert_eq!(repertoire[0].play_details[0].play_language, "Brak danych");
    assert_eq!(
        repertoire[0].lookup_metadata.movie_page_url.as_deref(),
        Some("https://helios.pl/warszawa/kino-helios-blue-city/filmy/imax-test-9001")
    );
    assert_eq!(
        repertoire[0].lookup_metadata.alternate_titles,
        vec!["Imax Test Original".to_string()]
    );
}

#[tokio::test]
async fn fetch_repertoire_uses_configured_base_url_for_metadata_links() {
    let custom_base_url = "https://mirror.helios.example";
    let mut evaluations = HashMap::new();
    evaluations.insert("repertoire".to_string(), fixture("lodz-repertoire-state.json"));
    evaluations.insert("current_cinema".to_string(), fixture("lodz-current-cinema.json"));
    let client = Helios::new(
        custom_base_url,
        DEFAULT_HELIOS_VENUES_URL,
        Arc::new(FakeRenderedPageRenderer { html: fixture("lodz-repertoire.html"), evaluations }),
    );

    let repertoire = client.fetch_repertoire("2026-04-01", &lodz_venue()).await.unwrap();
    let hopnieci = find_movie(&repertoire, "Hopnięci");

    assert_eq!(
        hopnieci.lookup_metadata.movie_page_url.as_deref(),
        Some("https://mirror.helios.example/lodz/kino-helios/filmy/hopnieci-4172")
    );
}

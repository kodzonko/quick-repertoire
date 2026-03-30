mod support;

use std::fs;
use std::sync::Arc;

use cinema_repertoire_analyzer::cinema::cinema_city::CinemaCity;
use cinema_repertoire_analyzer::cinema::registry::CinemaChainClient;
use cinema_repertoire_analyzer::domain::CinemaVenue;

use support::FakeHtmlRenderer;

#[tokio::test]
async fn fetch_repertoire_parses_saved_repertoire_snapshot() {
    let rendered_repertoire_html =
        fs::read_to_string("tests/resources/cinema_city_example_repertoire.html").unwrap();
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: rendered_repertoire_html,
        }),
    );
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1080".to_string(),
        venue_name: "Lodz - Manufaktura".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2023-04-01", &venue_data).await.unwrap();

    assert_eq!(repertoire[0].title, "65");
    assert_eq!(repertoire[0].genres, "N/A");
    assert_eq!(repertoire[0].play_length, "N/A");
    assert_eq!(repertoire[0].original_language, "EN");
    assert_eq!(repertoire[0].play_details[0].format, "2D");
    assert_eq!(repertoire[0].play_details[0].play_language, "NAP: PL");
    assert_eq!(
        repertoire[0].play_details[0].play_times,
        vec!["17:45".to_string(), "19:50".to_string()]
    );
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

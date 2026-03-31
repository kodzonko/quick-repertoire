mod support;

use std::sync::Arc;

use quick_repertoire::cinema::cinema_city::CinemaCity;
use quick_repertoire::cinema::registry::CinemaChainClient;
use quick_repertoire::domain::CinemaVenue;

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

#[tokio::test]
async fn fetch_repertoire_parses_inline_html_fixture_and_skips_presales() {
    let cinema = CinemaCity::new(
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}".to_string(),
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema".to_string(),
        Arc::new(FakeHtmlRenderer {
            html: rendered_repertoire_html(),
        }),
    );
    let venue_data = CinemaVenue {
        chain_id: "cinema-city".to_string(),
        venue_id: "1080".to_string(),
        venue_name: "Lodz - Manufaktura".to_string(),
    };

    let repertoire = cinema.fetch_repertoire("2023-04-01", &venue_data).await.unwrap();

    assert_eq!(repertoire.len(), 2);
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
    assert_eq!(repertoire[1].title, "Dungeons & Dragons");
    assert_eq!(repertoire[1].genres, "Fantasy, Adventure");
    assert_eq!(repertoire[1].play_length, "134 min");
    assert_eq!(repertoire[1].original_language, "EN");
    assert_eq!(repertoire[1].play_details[0].format, "IMAX 3D");
    assert_eq!(repertoire[1].play_details[0].play_language, "BEZ NAP");
    assert_eq!(repertoire[1].play_details[0].play_times, vec!["20:15".to_string()]);
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

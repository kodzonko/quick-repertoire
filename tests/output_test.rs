mod support;

use std::collections::HashMap;

use quick_repertoire::domain::{
    CinemaVenue, MoviePlayDetails, Repertoire, RepertoireCliTableMetadata, TmdbMovieDetails,
};
use quick_repertoire::output::{
    cinema_venue_input_parser, date_input_parser, render_repertoire_table, render_venues_table,
};

#[test]
fn cinema_venue_input_parser_parses_user_input_correctly() {
    assert_eq!(cinema_venue_input_parser(" Manufaktura "), "%Manufaktura%");
    assert_eq!(cinema_venue_input_parser("warszawa janki"), "%warszawa%janki%");
    assert_eq!(cinema_venue_input_parser("wroclawia."), "%wroclawia%");
}

#[test]
fn date_input_parser_accepts_supported_values() {
    assert_eq!(date_input_parser("2021-12-31").unwrap(), "2021-12-31");
    assert!(date_input_parser("today").is_ok());
    assert!(date_input_parser("jutro").is_ok());
}

#[test]
fn date_input_parser_rejects_unsupported_values() {
    assert_eq!(
        date_input_parser("foo").unwrap_err().to_string(),
        "Data: foo nie jest we wspieranym formacie: YYYY-MM-DD | dzis | jutro | itp..."
    );
}

#[test]
fn render_venues_table_prints_empty_state_for_missing_venues() {
    assert_eq!(render_venues_table(&[], "Cinema City"), "Brak kin tej sieci w bazie danych.");
}

#[test]
fn render_venues_table_renders_table_for_found_venues() {
    let rendered_output = render_venues_table(
        &[
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Lodz - Manufaktura".to_string(),
                venue_id: "1080".to_string(),
            },
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "1097".to_string(),
            },
        ],
        "Cinema City",
    );

    assert!(rendered_output.contains("Znalezione lokale sieci Cinema City"));
    assert!(rendered_output.contains("venue_name"));
    assert!(rendered_output.contains("Wroclaw - Wroclavia"));
}

#[test]
fn render_repertoire_table_prints_empty_state_for_missing_movies() {
    let metadata = RepertoireCliTableMetadata {
        chain_display_name: "Cinema City".to_string(),
        repertoire_date: "2024-06-01".to_string(),
        cinema_venue_name: "Wroclaw - Wroclavia".to_string(),
    };

    assert_eq!(
        render_repertoire_table(&[], &metadata, &HashMap::new()),
        "Brak repertuaru do wyświetlenia."
    );
}

#[test]
fn render_repertoire_table_renders_ratings_when_available() {
    let metadata = RepertoireCliTableMetadata {
        chain_display_name: "Cinema City".to_string(),
        repertoire_date: "2024-06-01".to_string(),
        cinema_venue_name: "Wroclaw - Wroclavia".to_string(),
    };
    let repertoire = vec![Repertoire {
        title: "Test Movie".to_string(),
        genres: "Thriller".to_string(),
        play_length: "120 min".to_string(),
        original_language: "EN".to_string(),
        play_details: vec![MoviePlayDetails {
            format: "2D".to_string(),
            play_language: "NAP: PL".to_string(),
            play_times: vec!["10:00".to_string(), "12:30".to_string()],
        }],
    }];
    let ratings = HashMap::from([(
        "Test Movie".to_string(),
        TmdbMovieDetails {
            rating: "8.5/10\n(głosy: 2000)".to_string(),
            summary: "A tense mystery.".to_string(),
        },
    )]);

    let rendered_output = render_repertoire_table(&repertoire, &metadata, &ratings);

    assert!(rendered_output.contains("Repertuar dla Cinema City (Wroclaw - Wroclavia)"));
    assert!(rendered_output.contains("Ocena z TMDB"));
    assert!(rendered_output.contains("8.5/10"));
    assert!(rendered_output.contains("A tense mystery."));
}

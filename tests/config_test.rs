mod support;

use std::fs;

use cinema_repertoire_analyzer::config::{
    load_settings, run_interactive_configuration, should_defer_bootstrap_to_command,
    should_skip_bootstrap_for_argv, write_settings,
};
use cinema_repertoire_analyzer::domain::CinemaVenue;
use cinema_repertoire_analyzer::error::AppError;
use cinema_repertoire_analyzer::persistence::DatabaseManager;
use support::{FakeCinemaClient, FakePrompt, FakeTmdbService, dependencies, settings};
use tempfile::tempdir;

#[test]
fn load_settings_roundtrips_config_ini() {
    let temp_dir = tempdir().unwrap();
    let mut expected_settings = settings(temp_dir.path());
    expected_settings.user_preferences.tmdb_access_token = Some("1234".to_string());

    write_settings(&expected_settings).unwrap();
    let loaded_settings = load_settings(temp_dir.path()).unwrap();

    assert_eq!(loaded_settings, expected_settings);
}

#[test]
fn load_settings_rejects_absolute_db_paths() {
    let temp_dir = tempdir().unwrap();
    fs::write(
        temp_dir.path().join("config.ini"),
        "[app]\n\
db_file = C:/absolute/test.sqlite\n\
loguru_level = INFO\n\
\n\
[user_preferences]\n\
default_chain = cinema-city\n\
default_day = today\n\
tmdb_access_token =\n\
\n\
[default_venues]\n\
cinema_city = Wroclaw - Wroclavia\n\
\n\
[cinema_chains.cinema_city]\n\
repertoire_url = https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_venue_id}&at={repertoire_date}\n\
venues_list_url = https://www.cinema-city.pl/#/buy-tickets-by-cinema\n",
    )
    .unwrap();

    assert_eq!(
        load_settings(temp_dir.path()).unwrap_err().to_string(),
        "Ścieżka pliku bazy danych w config.ini musi być względna wobec katalogu projektu."
    );
}

#[test]
fn bootstrap_rules_match_help_and_configure_flows() {
    assert!(should_skip_bootstrap_for_argv(&["--help".to_string()]));
    assert!(should_skip_bootstrap_for_argv(&[
        "venues".to_string(),
        "list".to_string(),
        "--help".to_string()
    ]));
    assert!(should_defer_bootstrap_to_command(&["configure".to_string()]));
    assert!(!should_defer_bootstrap_to_command(&["repertoire".to_string()]));
}

#[tokio::test]
async fn run_interactive_configuration_persists_selected_settings_and_venues() {
    let temp_dir = tempdir().unwrap();
    let cinema_client = FakeCinemaClient::new(
        Vec::new(),
        vec![
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Warszawa - Janki".to_string(),
                venue_id: "2".to_string(),
            },
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            },
        ],
    );
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(
            vec![
                "INFO".to_string(),
                "db.sqlite".to_string(),
                "cinema-city".to_string(),
                "today".to_string(),
                "Wroclaw - Wroclavia".to_string(),
            ],
            vec!["tmdb-token".to_string()],
        ),
        cinema_client,
        FakeTmdbService { result: Default::default(), error: None },
    );

    let configured_settings = run_interactive_configuration(
        temp_dir.path(),
        None,
        &dependencies.registry,
        dependencies.prompt.as_ref(),
    )
    .await
    .unwrap();

    assert_eq!(configured_settings.db_file, temp_dir.path().join("db.sqlite"));
    assert_eq!(
        configured_settings.user_preferences.tmdb_access_token,
        Some("tmdb-token".to_string())
    );
    assert_eq!(
        configured_settings
            .get_default_venue(cinema_repertoire_analyzer::domain::CinemaChainId::CinemaCity),
        Some("Wroclaw - Wroclavia")
    );
    assert!(temp_dir.path().join("config.ini").exists());

    let persisted_db_manager = DatabaseManager::new(configured_settings.db_file.clone()).unwrap();
    assert_eq!(
        persisted_db_manager
            .get_all_venues("cinema-city")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![
            ("Warszawa - Janki".to_string(), "2".to_string()),
            ("Wroclaw - Wroclavia".to_string(), "3".to_string())
        ]
    );
}

#[tokio::test]
async fn run_interactive_configuration_does_not_create_config_when_venue_fetch_fails() {
    let temp_dir = tempdir().unwrap();
    let mut cinema_client = FakeCinemaClient::new(Vec::new(), Vec::new());
    cinema_client.venues_error = Some(AppError::configuration("boom"));
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(vec!["INFO".to_string(), "db.sqlite".to_string()], Vec::new()),
        cinema_client,
        FakeTmdbService { result: Default::default(), error: None },
    );

    let error = run_interactive_configuration(
        temp_dir.path(),
        None,
        &dependencies.registry,
        dependencies.prompt.as_ref(),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Nie udało się pobrać list lokali dla wszystkich sieci. Niepowodzenie: Cinema City."
    );
    assert!(!temp_dir.path().join("config.ini").exists());
}

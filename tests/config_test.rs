mod support;

use std::fs;

use quick_repertoire::config::{
    AppPaths, load_settings, run_interactive_configuration, should_defer_bootstrap_to_command,
    should_skip_bootstrap_for_argv, write_settings,
};
use quick_repertoire::domain::CinemaVenue;
use quick_repertoire::error::AppError;
use quick_repertoire::persistence::DatabaseManager;
use support::{FakeCinemaClient, FakePrompt, FakeTmdbService, dependencies, settings};
use tempfile::tempdir;

#[test]
fn load_settings_roundtrips_config_ini() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let mut expected_settings = settings();
    expected_settings.user_preferences.tmdb_access_token = Some("1234".to_string());

    write_settings(&expected_settings, &paths).unwrap();
    let loaded_settings = load_settings(&paths).unwrap();

    assert_eq!(loaded_settings, expected_settings);
}

#[test]
fn load_settings_ignores_legacy_db_file_entry() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(
        paths.config_file(),
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

    assert_eq!(load_settings(&paths).unwrap().loguru_level, "INFO");
}

#[test]
fn write_settings_omits_db_file_entry() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());

    write_settings(&settings(), &paths).unwrap();

    assert!(!fs::read_to_string(paths.config_file()).unwrap().contains("db_file ="));
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
        &dependencies.paths,
        None,
        &dependencies.registry,
        dependencies.prompt.as_ref(),
    )
    .await
    .unwrap();

    assert_eq!(
        configured_settings.user_preferences.tmdb_access_token,
        Some("tmdb-token".to_string())
    );
    assert_eq!(
        configured_settings.get_default_venue(quick_repertoire::domain::CinemaChainId::CinemaCity),
        Some("Wroclaw - Wroclavia")
    );
    assert!(dependencies.paths.config_file().exists());
    assert!(dependencies.paths.db_file().exists());

    let persisted_db_manager = DatabaseManager::new(dependencies.paths.db_file()).unwrap();
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
        FakePrompt::new(vec!["INFO".to_string()], Vec::new()),
        cinema_client,
        FakeTmdbService { result: Default::default(), error: None },
    );

    let error = run_interactive_configuration(
        &dependencies.paths,
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
    assert!(!dependencies.paths.config_file().exists());
    assert!(!dependencies.paths.db_file().exists());
}

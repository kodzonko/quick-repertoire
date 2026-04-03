mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use quick_repertoire::config::{
    AppPaths, DEFAULT_DEVELOPMENT_LOG_LEVEL, DEFAULT_PRODUCTION_LOG_LEVEL, default_log_level,
    load_settings, run_interactive_configuration, should_defer_bootstrap_to_command,
    should_skip_bootstrap_for_argv, write_settings,
};
use quick_repertoire::domain::{CinemaChainId, CinemaVenue};
use quick_repertoire::error::AppError;
use quick_repertoire::persistence::DatabaseManager;
use support::{
    AcceptDefaultsPrompt, ConcurrencyTracker, DelayedCinemaClient, FakeCinemaClient, FakePrompt,
    FakeTmdbService, delayed_registered_chain, dependencies, dependencies_with_chains,
    dependencies_with_prompt_adapter, registered_chain, settings,
};
use tempfile::tempdir;

#[test]
fn load_settings_roundtrips_config_ini() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let mut expected_settings = settings();
    expected_settings.user_preferences.tmdb_access_token = Some("1234".to_string());
    expected_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Helios, Some("Łódź - Helios".to_string()));
    expected_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Multikino, Some("Warszawa Złote Tarasy".to_string()));

    write_settings(&expected_settings, &paths).unwrap();
    let loaded_settings = load_settings(&paths).unwrap();

    assert_eq!(loaded_settings, expected_settings);
}

#[test]
fn load_settings_ignores_legacy_app_section_entries() {
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

    let loaded_settings = load_settings(&paths).unwrap();
    assert_eq!(
        loaded_settings.user_preferences.default_chain,
        quick_repertoire::domain::CinemaChainId::CinemaCity
    );
    assert_eq!(loaded_settings.user_preferences.default_day, "dziś");
}

#[test]
fn load_settings_reads_default_venues_for_multiple_chains() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(
        paths.config_file(),
        "[user_preferences]\n\
default_chain = helios\n\
default_day = jutro\n\
tmdb_access_token = token\n\
\n\
[default_venues]\n\
cinema_city = Wroclaw - Wroclavia\n\
helios = Łódź - Helios\n\
multikino = Warszawa Złote Tarasy\n\
",
    )
    .unwrap();

    let loaded_settings = load_settings(&paths).unwrap();

    assert_eq!(loaded_settings.user_preferences.default_chain, CinemaChainId::Helios);
    assert_eq!(
        loaded_settings.get_default_venue(CinemaChainId::CinemaCity),
        Some("Wroclaw - Wroclavia")
    );
    assert_eq!(loaded_settings.get_default_venue(CinemaChainId::Helios), Some("Łódź - Helios"));
    assert_eq!(
        loaded_settings.get_default_venue(CinemaChainId::Multikino),
        Some("Warszawa Złote Tarasy")
    );
}

#[test]
fn write_settings_omits_legacy_app_entries() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());

    write_settings(&settings(), &paths).unwrap();

    let config = fs::read_to_string(paths.config_file()).unwrap();
    assert!(!config.contains("db_file ="));
    assert!(!config.contains("loguru_level ="));
    assert!(!config.contains("[cinema_chains.cinema_city]"));
    assert!(!config.contains("repertoire_url ="));
    assert!(!config.contains("venues_list_url ="));
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
    assert!(should_defer_bootstrap_to_command(&["chains".to_string()]));
    assert!(!should_defer_bootstrap_to_command(&["repertoire".to_string()]));
}

#[test]
fn load_settings_reports_recovery_command_with_binary_name() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(paths.config_file(), "[app\n").unwrap();

    let error = load_settings(&paths).unwrap_err();

    assert!(error.to_string().contains("`quickrep configure`"));
}

#[test]
fn default_log_level_matches_current_build_profile() {
    let expected = if cfg!(debug_assertions) {
        DEFAULT_DEVELOPMENT_LOG_LEVEL
    } else {
        DEFAULT_PRODUCTION_LOG_LEVEL
    };
    assert_eq!(default_log_level(), expected);
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
            vec!["cinema-city".to_string(), "dziś".to_string(), "Wroclaw - Wroclavia".to_string()],
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
async fn run_interactive_configuration_preserves_existing_default_venue_for_other_chain() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(
            vec!["helios".to_string(), "jutro".to_string(), "Łódź - Helios".to_string()],
            vec!["tmdb-token".to_string()],
        ),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(
                    Vec::new(),
                    vec![CinemaVenue {
                        chain_id: "cinema-city".to_string(),
                        venue_name: "Wroclaw - Wroclavia".to_string(),
                        venue_id: "3".to_string(),
                    }],
                ),
            ),
            registered_chain(
                CinemaChainId::Helios,
                "Helios",
                FakeCinemaClient::new(
                    Vec::new(),
                    vec![
                        CinemaVenue {
                            chain_id: "helios".to_string(),
                            venue_name: "Gdynia - Helios".to_string(),
                            venue_id: "gdynia/kino-helios".to_string(),
                        },
                        CinemaVenue {
                            chain_id: "helios".to_string(),
                            venue_name: "Łódź - Helios".to_string(),
                            venue_id: "lodz/kino-helios".to_string(),
                        },
                    ],
                ),
            ),
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );

    let configured_settings = run_interactive_configuration(
        &dependencies.paths,
        Some(settings()),
        &dependencies.registry,
        dependencies.prompt.as_ref(),
    )
    .await
    .unwrap();

    assert_eq!(configured_settings.user_preferences.default_chain, CinemaChainId::Helios);
    assert_eq!(
        configured_settings.get_default_venue(CinemaChainId::CinemaCity),
        Some("Wroclaw - Wroclavia")
    );
    assert_eq!(configured_settings.get_default_venue(CinemaChainId::Helios), Some("Łódź - Helios"));

    let persisted_db_manager = DatabaseManager::new(dependencies.paths.db_file()).unwrap();
    assert_eq!(
        persisted_db_manager
            .get_all_venues("helios")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![
            ("Gdynia - Helios".to_string(), "gdynia/kino-helios".to_string()),
            ("Łódź - Helios".to_string(), "lodz/kino-helios".to_string())
        ]
    );
}

#[tokio::test]
async fn run_interactive_configuration_fetches_all_chain_venues_concurrently() {
    let temp_dir = tempdir().unwrap();
    let tracker = Arc::new(ConcurrencyTracker::default());
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        AcceptDefaultsPrompt::new(Vec::new(), Vec::new()),
        vec![
            delayed_registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                DelayedCinemaClient {
                    venues: vec![CinemaVenue {
                        chain_id: "cinema-city".to_string(),
                        venue_name: "Wroclaw - Wroclavia".to_string(),
                        venue_id: "3".to_string(),
                    }],
                    delay: Duration::from_millis(75),
                    tracker: tracker.clone(),
                },
            ),
            delayed_registered_chain(
                CinemaChainId::Helios,
                "Helios",
                DelayedCinemaClient {
                    venues: vec![CinemaVenue {
                        chain_id: "helios".to_string(),
                        venue_name: "Łódź - Helios".to_string(),
                        venue_id: "lodz/kino-helios".to_string(),
                    }],
                    delay: Duration::from_millis(75),
                    tracker: tracker.clone(),
                },
            ),
            delayed_registered_chain(
                CinemaChainId::Multikino,
                "Multikino",
                DelayedCinemaClient {
                    venues: vec![CinemaVenue {
                        chain_id: "multikino".to_string(),
                        venue_name: "Warszawa Złote Tarasy".to_string(),
                        venue_id: "0034".to_string(),
                    }],
                    delay: Duration::from_millis(75),
                    tracker: tracker.clone(),
                },
            ),
        ],
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

    assert_eq!(tracker.max_in_flight(), 3);
    assert_eq!(
        configured_settings.get_default_venue(CinemaChainId::CinemaCity),
        Some("Wroclaw - Wroclavia")
    );
    assert_eq!(
        DatabaseManager::new(dependencies.paths.db_file())
            .unwrap()
            .get_all_venues("helios")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![("Łódź - Helios".to_string(), "lodz/kino-helios".to_string())]
    );
    assert_eq!(
        DatabaseManager::new(dependencies.paths.db_file())
            .unwrap()
            .get_all_venues("multikino")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![("Warszawa Złote Tarasy".to_string(), "0034".to_string())]
    );
}

#[tokio::test]
async fn run_interactive_configuration_defaults_venue_to_first_sorted_option() {
    let temp_dir = tempdir().unwrap();
    let cinema_client = FakeCinemaClient::new(
        Vec::new(),
        vec![
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            },
            CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Lodz - Manufaktura".to_string(),
                venue_id: "2".to_string(),
            },
        ],
    );
    let dependencies = dependencies_with_prompt_adapter(
        temp_dir.path(),
        AcceptDefaultsPrompt::new(Vec::new(), Vec::new()),
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
        configured_settings.get_default_venue(quick_repertoire::domain::CinemaChainId::CinemaCity),
        Some("Lodz - Manufaktura")
    );
}

#[tokio::test]
async fn run_interactive_configuration_does_not_create_config_when_venue_fetch_fails() {
    let temp_dir = tempdir().unwrap();
    let mut cinema_client = FakeCinemaClient::new(Vec::new(), Vec::new());
    cinema_client.venues_error = Some(AppError::configuration("boom"));
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
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

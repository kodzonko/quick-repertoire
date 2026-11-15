mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;

use quick_repertoire::config::{
    AppPaths, Settings, load_settings, run_interactive_configuration, write_settings,
};
use quick_repertoire::domain::{CinemaChainId, CinemaVenue};
use quick_repertoire::error::AppError;
use quick_repertoire::persistence::DatabaseManager;
use support::{
    AcceptDefaultsPrompt, ConcurrencyTracker, DelayedCinemaClient, FakeCinemaClient, FakePrompt,
    FakeTmdbService, delayed_registered_chain, dependencies, dependencies_with_chains,
    registered_chain, settings,
};
use tempfile::tempdir;

#[test]
fn load_settings_roundtrips_sqlite_without_creating_config_files() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let mut expected = settings();
    expected.user_preferences.tmdb_access_token = Some("token\"\\\n1234".to_string());
    expected.user_preferences.default_chain = CinemaChainId::Helios;
    expected
        .user_preferences
        .default_venues
        .set(CinemaChainId::Helios, Some("Łódź - Helios".into()));
    expected
        .user_preferences
        .default_venues
        .set(CinemaChainId::Multikino, Some("Warszawa Złote Tarasy".into()));

    write_settings(&expected, &paths).unwrap();

    assert!(paths.db_file().exists());
    assert_eq!(load_settings(&paths).unwrap(), expected);
    assert_eq!(fs::read_dir(temp_dir.path()).unwrap().count(), 1);
}

#[test]
fn write_settings_replaces_preferences_and_clears_removed_values() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    write_settings(&settings(), &paths).unwrap();
    write_settings(&Settings::default(), &paths).unwrap();

    assert_eq!(load_settings(&paths).unwrap(), Settings::default());
}

#[test]
fn settings_and_venue_refresh_preserve_each_other() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let database = DatabaseManager::new(paths.db_file()).unwrap();
    let venue = CinemaVenue {
        chain_id: "helios".into(),
        venue_id: "lodz".into(),
        venue_name: "Łódź - Helios".into(),
    };
    database.replace_venues(CinemaChainId::Helios, std::slice::from_ref(&venue)).unwrap();
    write_settings(&settings(), &paths).unwrap();
    assert_eq!(database.get_all_venues("helios").unwrap(), vec![venue]);

    database.replace_venues(CinemaChainId::Helios, &[]).unwrap();
    assert_eq!(load_settings(&paths).unwrap(), settings());
}

#[test]
fn load_settings_migrates_json_into_existing_venue_database() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let venue = CinemaVenue {
        chain_id: "helios".into(),
        venue_id: "lodz".into(),
        venue_name: "Łódź - Helios".into(),
    };
    let connection = rusqlite::Connection::open(paths.db_file()).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE cinema_venues (
                chain_id TEXT NOT NULL, venue_id TEXT NOT NULL, venue_name TEXT NOT NULL,
                PRIMARY KEY (chain_id, venue_id)
            );",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO cinema_venues VALUES (?1, ?2, ?3)",
            rusqlite::params![venue.chain_id, venue.venue_id, venue.venue_name],
        )
        .unwrap();
    drop(connection);
    let legacy_path = temp_dir.path().join("config.json");
    let content = r#"{"user_preferences": {
        "default_chain": "helios", "default_day": " tomorrow ", "tmdb_access_token": " token ",
        "default_venues": {"helios": " Łódź - Helios ", "cinema_city": " "}
    }}"#;
    fs::write(&legacy_path, content).unwrap();
    fs::write(temp_dir.path().join("config.ini"), "invalid older config").unwrap();

    let loaded = load_settings(&paths).unwrap();

    assert_eq!(loaded.user_preferences.default_chain, CinemaChainId::Helios);
    assert_eq!(loaded.user_preferences.default_day, "jutro");
    assert_eq!(loaded.user_preferences.tmdb_access_token.as_deref(), Some("token"));
    assert_eq!(loaded.get_default_venue(CinemaChainId::Helios), Some("Łódź - Helios"));
    assert_eq!(loaded.get_default_venue(CinemaChainId::CinemaCity), None);
    assert_eq!(
        DatabaseManager::new(paths.db_file()).unwrap().get_all_venues("helios").unwrap(),
        vec![venue]
    );
    assert_eq!(fs::read_to_string(&legacy_path).unwrap(), content);
    fs::remove_file(legacy_path).unwrap();
    assert_eq!(load_settings(&paths).unwrap(), loaded);
}

#[test]
fn load_settings_migrates_ini_with_all_preferences_and_ignores_legacy_app_entries() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let legacy_path = temp_dir.path().join("config.ini");
    let content = "[app]\ndb_file = ignored.sqlite\n\
[user_preferences]\ndefault_chain = helios\ndefault_day = tomorrow\ntmdb_access_token = token\n\
[default_venues]\ncinema_city = Wroclaw - Wroclavia\nhelios = Łódź - Helios\nmultikino = Warszawa Złote Tarasy\n";
    fs::write(&legacy_path, content).unwrap();

    let loaded = load_settings(&paths).unwrap();

    assert!(paths.db_file().exists());
    assert_eq!(loaded.user_preferences.default_chain, CinemaChainId::Helios);
    assert_eq!(loaded.user_preferences.default_day, "jutro");
    assert_eq!(loaded.user_preferences.tmdb_access_token.as_deref(), Some("token"));
    assert_eq!(loaded.get_default_venue(CinemaChainId::CinemaCity), Some("Wroclaw - Wroclavia"));
    assert_eq!(loaded.get_default_venue(CinemaChainId::Helios), Some("Łódź - Helios"));
    assert_eq!(loaded.get_default_venue(CinemaChainId::Multikino), Some("Warszawa Złote Tarasy"));
    assert_eq!(fs::read_to_string(legacy_path).unwrap(), content);
    assert!(!temp_dir.path().join("config.json").exists());
    assert_eq!(load_settings(&paths).unwrap(), loaded);
}

#[test]
fn load_settings_defaults_missing_fields_when_migrating_json() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(temp_dir.path().join("config.json"), r#"{"user_preferences":{}}"#).unwrap();
    assert_eq!(load_settings(&paths).unwrap(), Settings::default());
}

#[test]
fn load_settings_prefers_sqlite_over_legacy_files() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    write_settings(&settings(), &paths).unwrap();
    fs::write(temp_dir.path().join("config.json"), "invalid json").unwrap();
    fs::write(temp_dir.path().join("config.ini"), "invalid ini").unwrap();

    assert_eq!(load_settings(&paths).unwrap(), settings());
}

#[test]
fn load_settings_rejects_invalid_json_without_falling_back_to_ini_or_leaking_values() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(temp_dir.path().join("config.ini"), "[user_preferences]\ndefault_chain = helios\n")
        .unwrap();
    for json in [
        "{",
        "{}",
        "[]",
        r#"{"user_preferences":{"default_chain":"secret-token"}}"#,
        r#"{"user_preferences":{"default_day":123}}"#,
        r#"{"user_preferences":{"tmdb_access_token":123}}"#,
        r#"{"user_preferences":{"default_venues":{"helios":123}}}"#,
    ] {
        fs::write(temp_dir.path().join("config.json"), json).unwrap();
        let message = load_settings(&paths).unwrap_err().to_string();
        assert!(message.contains("`quickrep configure`"));
        assert!(!message.contains("secret-token"));
        assert!(!paths.db_file().exists());
    }
}

#[test]
fn load_settings_reports_missing_configuration_without_creating_database() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());

    assert_eq!(load_settings(&paths).unwrap_err(), AppError::ConfigurationNotFound);
    assert!(!paths.db_file().exists());
}

#[test]
fn load_settings_treats_venue_only_database_as_unconfigured() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    DatabaseManager::new(paths.db_file()).unwrap();

    assert_eq!(load_settings(&paths).unwrap_err(), AppError::ConfigurationNotFound);
}

#[test]
fn load_settings_preserves_invalid_legacy_configuration() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    let legacy_path = temp_dir.path().join("config.ini");
    fs::write(&legacy_path, "[broken\n").unwrap();

    assert!(load_settings(&paths).unwrap_err().to_string().contains("`quickrep configure`"));
    assert_eq!(fs::read_to_string(legacy_path).unwrap(), "[broken\n");
    assert!(!paths.db_file().exists());
}

#[test]
fn load_settings_preserves_corrupted_database_without_importing_legacy_files() {
    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    fs::write(paths.db_file(), "broken database").unwrap();
    fs::write(temp_dir.path().join("config.json"), r#"{"user_preferences":{}}"#).unwrap();

    assert!(load_settings(&paths).is_err());
    assert_eq!(fs::read_to_string(paths.db_file()).unwrap(), "broken database");
}

#[cfg(unix)]
#[test]
fn write_settings_restricts_new_and_existing_database_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempdir().unwrap();
    let paths = AppPaths::for_runtime_dir(temp_dir.path().to_path_buf());
    write_settings(&settings(), &paths).unwrap();
    assert_eq!(fs::metadata(paths.db_file()).unwrap().permissions().mode() & 0o777, 0o600);
    fs::set_permissions(paths.db_file(), fs::Permissions::from_mode(0o644)).unwrap();
    write_settings(&settings(), &paths).unwrap();
    assert_eq!(fs::metadata(paths.db_file()).unwrap().permissions().mode() & 0o777, 0o600);
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
    assert!(dependencies.paths.db_file().exists());
    assert_eq!(load_settings(&dependencies.paths).unwrap(), configured_settings);

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
    let dependencies = dependencies(
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
    assert!(!dependencies.paths.db_file().exists());
}

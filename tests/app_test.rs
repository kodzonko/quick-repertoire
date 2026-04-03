mod support;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::Command;
use quick_repertoire::app::run_with_args;
use quick_repertoire::config::write_settings;
use quick_repertoire::domain::{
    CinemaChainId, CinemaVenue, MovieLookupMetadata, MoviePlayDetails, MoviePlayTime, Repertoire,
};
use quick_repertoire::error::AppError;
use quick_repertoire::output::BufferTerminal;
use quick_repertoire::persistence::DatabaseManager;
use support::{
    ConcurrencyTracker, DelayedCinemaClient, FailingWriteAccessProbe, FakeCinemaClient, FakePrompt,
    FakeTmdbService, delayed_registered_chain, dependencies, dependencies_with_chains,
    dependencies_with_write_access_probe, registered_chain, settings,
};
use tempfile::tempdir;

#[test]
fn binary_help_lists_top_level_commands() {
    Command::cargo_bin("quickrep")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("quickrep"))
        .stdout(predicates::str::contains("Użycie"))
        .stdout(predicates::str::contains("Polecenia"))
        .stdout(predicates::str::contains("Opcje"))
        .stdout(predicates::str::contains("chains"))
        .stdout(predicates::str::contains("configure"))
        .stdout(predicates::str::contains("repertoire"))
        .stdout(predicates::str::contains("venues"));
}

#[tokio::test]
async fn repertoire_command_exits_for_unsupported_chain() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(Vec::new(), Vec::new()),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "repertoire".to_string(),
            "--chain".to_string(),
            "unsupported-chain".to_string(),
        ],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 1);
    assert!(terminal.into_string().contains("Nieobsługiwana sieć kin"));
}

#[tokio::test]
async fn chains_command_lists_supported_chains_without_configuration() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
            ),
            registered_chain(
                CinemaChainId::Helios,
                "Helios",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
            ),
            registered_chain(
                CinemaChainId::Multikino,
                "Multikino",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
            ),
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "chains".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Obsługiwane sieci kin"));
    assert!(output.contains("Cinema City"));
    assert!(output.contains("cinema-city"));
    assert!(output.contains("Helios"));
    assert!(output.contains("Multikino"));
    assert!(!dependencies.paths.config_file().exists());
}

#[tokio::test]
async fn repertoire_command_uses_default_chain_and_default_venue_when_name_not_provided() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(
            vec![Repertoire {
                title: "Test Movie".to_string(),
                genres: "Thriller".to_string(),
                play_length: "120 min".to_string(),
                original_language: "EN".to_string(),
                play_details: vec![MoviePlayDetails {
                    format: "2D".to_string(),
                    play_language: "NAP: PL".to_string(),
                    play_times: vec![
                        MoviePlayTime { value: "10:00".to_string(), url: None },
                        MoviePlayTime { value: "12:30".to_string(), url: None },
                    ],
                }],
                lookup_metadata: MovieLookupMetadata::default(),
            }],
            Vec::new(),
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Repertuar dla Cinema City"));
    assert!(output.contains("Wroclaw - Wroclavia"));
    assert!(output.contains("Test Movie"));
}

#[tokio::test]
async fn repertoire_command_warns_when_tmdb_is_disabled() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(
            vec![Repertoire {
                title: "Test Movie".to_string(),
                genres: "Thriller".to_string(),
                play_length: "120 min".to_string(),
                original_language: "EN".to_string(),
                play_details: vec![MoviePlayDetails {
                    format: "2D".to_string(),
                    play_language: "NAP: PL".to_string(),
                    play_times: vec![MoviePlayTime { value: "10:00".to_string(), url: None }],
                }],
                lookup_metadata: MovieLookupMetadata::default(),
            }],
            Vec::new(),
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut settings = settings();
    settings.user_preferences.tmdb_access_token = None;
    write_settings(&settings, &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string(), "wroclavia".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Klucz API do usługi TMDB nie jest skonfigurowany"));
    assert!(output.contains("Test Movie"));
}

#[tokio::test]
async fn repertoire_command_supports_helios_chain() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
            ),
            registered_chain(
                CinemaChainId::Helios,
                "Helios",
                FakeCinemaClient::new(
                    vec![Repertoire {
                        title: "Helios Movie".to_string(),
                        genres: "Dramat".to_string(),
                        play_length: "101 min".to_string(),
                        original_language: "Brak danych".to_string(),
                        play_details: vec![MoviePlayDetails {
                            format: "Dream 2D Atmos".to_string(),
                            play_language: "Napisy".to_string(),
                            play_times: vec![MoviePlayTime {
                                value: "18:30".to_string(),
                                url: Some("https://bilety.helios.pl/screen/test".to_string()),
                            }],
                        }],
                        lookup_metadata: MovieLookupMetadata::default(),
                    }],
                    Vec::new(),
                ),
            ),
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut configured_settings = settings();
    configured_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Helios, Some("Łódź - Helios".to_string()));
    write_settings(&configured_settings, &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "helios",
            &[CinemaVenue {
                chain_id: "helios".to_string(),
                venue_name: "Łódź - Helios".to_string(),
                venue_id: "lodz/kino-helios".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "repertoire".to_string(),
            "--chain".to_string(),
            "helios".to_string(),
        ],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Repertuar dla Helios"));
    assert!(output.contains("Łódź - Helios"));
    assert!(output.contains("Helios Movie"));
}

#[tokio::test]
async fn repertoire_command_supports_multikino_chain() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
            ),
            registered_chain(
                CinemaChainId::Multikino,
                "Multikino",
                FakeCinemaClient::new(
                    vec![Repertoire {
                        title: "Multikino Movie".to_string(),
                        genres: "science fiction".to_string(),
                        play_length: "157 min".to_string(),
                        original_language: "Brak danych".to_string(),
                        play_details: vec![MoviePlayDetails {
                            format: "2D".to_string(),
                            play_language: "NAPISY".to_string(),
                            play_times: vec![MoviePlayTime {
                                value: "19:30".to_string(),
                                url: Some(
                                    "https://www.multikino.pl/rezerwacja-biletow/podsumowanie/0034/HO00002328/64812"
                                        .to_string(),
                                ),
                            }],
                        }],
                        lookup_metadata: MovieLookupMetadata::default(),
                    }],
                    Vec::new(),
                ),
            ),
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut configured_settings = settings();
    configured_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Multikino, Some("Warszawa Złote Tarasy".to_string()));
    write_settings(&configured_settings, &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "multikino",
            &[CinemaVenue {
                chain_id: "multikino".to_string(),
                venue_name: "Warszawa Złote Tarasy".to_string(),
                venue_id: "0034".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "repertoire".to_string(),
            "--chain".to_string(),
            "multikino".to_string(),
        ],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Repertuar dla Multikino"));
    assert!(output.contains("Warszawa Złote Tarasy"));
    assert!(output.contains("Multikino Movie"));
}

#[tokio::test]
async fn venues_update_without_chain_refreshes_all_supported_chains_concurrently() {
    let temp_dir = tempdir().unwrap();
    let tracker = Arc::new(ConcurrencyTracker::default());
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            delayed_registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                DelayedCinemaClient {
                    venues: vec![CinemaVenue {
                        chain_id: "cinema-city".to_string(),
                        venue_name: "Test Venue".to_string(),
                        venue_id: "9999".to_string(),
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
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "venues".to_string(), "update".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 0);
    let output = terminal.into_string();
    assert!(output.contains("Aktualizowanie lokali dla wszystkich obsługiwanych sieci..."));
    assert!(output.contains(
        "Lokale wszystkich obsługiwanych sieci zostały zaktualizowane w lokalnej bazie danych."
    ));
    assert_eq!(tracker.max_in_flight(), 2);
    assert_eq!(
        DatabaseManager::new(dependencies.paths.db_file())
            .unwrap()
            .get_all_venues("cinema-city")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![("Test Venue".to_string(), "9999".to_string())]
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
}

#[tokio::test]
async fn venues_commands_support_helios_chain() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(Vec::new(), Vec::new()),
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
    let mut configured_settings = settings();
    configured_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Helios, Some("Łódź - Helios".to_string()));
    write_settings(&configured_settings, &dependencies.paths).unwrap();

    let mut update_terminal = BufferTerminal::default();
    let update_exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "venues".to_string(),
            "update".to_string(),
            "--chain".to_string(),
            "helios".to_string(),
        ],
        &dependencies,
        &mut update_terminal,
    )
    .await;
    assert_eq!(update_exit_code, 0);
    assert!(update_terminal.into_string().contains("Aktualizowanie lokali dla sieci: Helios..."));

    let mut list_terminal = BufferTerminal::default();
    let list_exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "venues".to_string(),
            "list".to_string(),
            "--chain".to_string(),
            "helios".to_string(),
        ],
        &dependencies,
        &mut list_terminal,
    )
    .await;
    let list_output = list_terminal.into_string();
    assert_eq!(list_exit_code, 0);
    assert!(list_output.contains("Znalezione lokale sieci Helios"));
    assert!(list_output.contains("Gdynia - Helios"));
    assert!(list_output.contains("Łódź - Helios"));

    let mut search_terminal = BufferTerminal::default();
    let search_exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "venues".to_string(),
            "search".to_string(),
            "łódź".to_string(),
            "--chain".to_string(),
            "helios".to_string(),
        ],
        &dependencies,
        &mut search_terminal,
    )
    .await;
    let search_output = search_terminal.into_string();
    assert_eq!(search_exit_code, 0);
    assert!(search_output.contains("Łódź - Helios"));
    assert!(!search_output.contains("Gdynia - Helios"));
}

#[tokio::test]
async fn configure_command_uses_existing_settings_when_available() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(
            vec!["cinema-city".to_string(), "dziś".to_string(), "Wroclaw - Wroclavia".to_string()],
            vec!["tmdb-token".to_string()],
        ),
        FakeCinemaClient::new(
            Vec::new(),
            vec![CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        ),
        FakeTmdbService { result: HashMap::new(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "configure".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 0);
    assert!(terminal.into_string().contains("Konfiguracja zapisana w config.ini."));
    assert!(fs::read_to_string(dependencies.paths.config_file()).unwrap().contains("tmdb-token"));
}

#[tokio::test]
async fn repertoire_command_warns_when_tmdb_lookup_fails() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(
            vec![Repertoire {
                title: "Test Movie".to_string(),
                genres: "Thriller".to_string(),
                play_length: "120 min".to_string(),
                original_language: "EN".to_string(),
                play_details: vec![MoviePlayDetails {
                    format: "2D".to_string(),
                    play_language: "NAP: PL".to_string(),
                    play_times: vec![MoviePlayTime { value: "10:00".to_string(), url: None }],
                }],
                lookup_metadata: MovieLookupMetadata::default(),
            }],
            Vec::new(),
        ),
        FakeTmdbService {
            result: Default::default(),
            error: Some(AppError::Http("boom".to_string())),
        },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string(), "wroclavia".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Nie udało się pobrać danych z usługi TMDB"));
    assert!(output.contains("Test Movie"));
}

#[tokio::test]
async fn repertoire_command_bootstraps_configuration_before_running() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(
            vec!["cinema-city".to_string(), "dziś".to_string(), "Wroclaw - Wroclavia".to_string()],
            vec!["tmdb-token".to_string()],
        ),
        FakeCinemaClient::new(
            vec![Repertoire {
                title: "Test Movie".to_string(),
                genres: "Thriller".to_string(),
                play_length: "120 min".to_string(),
                original_language: "EN".to_string(),
                play_details: vec![MoviePlayDetails {
                    format: "2D".to_string(),
                    play_language: "NAP: PL".to_string(),
                    play_times: vec![
                        MoviePlayTime { value: "10:00".to_string(), url: None },
                        MoviePlayTime { value: "12:30".to_string(), url: None },
                    ],
                }],
                lookup_metadata: MovieLookupMetadata::default(),
            }],
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
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(dependencies.paths.config_file().exists());
    assert!(dependencies.paths.db_file().exists());
    assert!(output.contains("Repertuar dla Cinema City"));
    assert!(output.contains("Wroclaw - Wroclavia"));
    assert!(output.contains("Test Movie"));
    assert!(fs::read_to_string(dependencies.paths.config_file()).unwrap().contains("tmdb-token"));
}

#[tokio::test]
async fn repertoire_command_rehydrates_missing_database_before_running() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_chains(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        vec![
            registered_chain(
                CinemaChainId::CinemaCity,
                "Cinema City",
                FakeCinemaClient::new(
                    vec![Repertoire {
                        title: "Test Movie".to_string(),
                        genres: "Thriller".to_string(),
                        play_length: "120 min".to_string(),
                        original_language: "EN".to_string(),
                        play_details: vec![MoviePlayDetails {
                            format: "2D".to_string(),
                            play_language: "NAP: PL".to_string(),
                            play_times: vec![
                                MoviePlayTime { value: "10:00".to_string(), url: None },
                                MoviePlayTime { value: "12:30".to_string(), url: None },
                            ],
                        }],
                        lookup_metadata: MovieLookupMetadata::default(),
                    }],
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
                ),
            ),
            registered_chain(
                CinemaChainId::Helios,
                "Helios",
                FakeCinemaClient::new(
                    Vec::new(),
                    vec![CinemaVenue {
                        chain_id: "helios".to_string(),
                        venue_name: "Łódź - Helios".to_string(),
                        venue_id: "lodz/kino-helios".to_string(),
                    }],
                ),
            ),
        ],
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut configured_settings = settings();
    configured_settings
        .user_preferences
        .default_venues
        .set(CinemaChainId::Helios, Some("Łódź - Helios".to_string()));
    write_settings(&configured_settings, &dependencies.paths).unwrap();
    assert!(!dependencies.paths.db_file().exists());
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 0);
    assert!(output.contains("Repertuar dla Cinema City"));
    assert!(output.contains("Wroclaw - Wroclavia"));
    assert!(output.contains("Test Movie"));
    assert!(dependencies.paths.db_file().exists());
    assert_eq!(
        DatabaseManager::new(dependencies.paths.db_file())
            .unwrap()
            .get_all_venues("cinema-city")
            .unwrap()
            .into_iter()
            .map(|venue| (venue.venue_name, venue.venue_id))
            .collect::<Vec<_>>(),
        vec![
            ("Warszawa - Janki".to_string(), "2".to_string()),
            ("Wroclaw - Wroclavia".to_string(), "3".to_string()),
        ]
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
}

#[tokio::test]
async fn venues_commands_update_list_and_search_round_trip_through_database() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(
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
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();

    let mut update_terminal = BufferTerminal::default();
    let update_exit_code = run_with_args(
        vec!["quickrep".to_string(), "venues".to_string(), "update".to_string()],
        &dependencies,
        &mut update_terminal,
    )
    .await;
    assert_eq!(update_exit_code, 0);

    let mut list_terminal = BufferTerminal::default();
    let list_exit_code = run_with_args(
        vec!["quickrep".to_string(), "venues".to_string(), "list".to_string()],
        &dependencies,
        &mut list_terminal,
    )
    .await;
    let list_output = list_terminal.into_string();
    assert_eq!(list_exit_code, 0);
    assert!(list_output.contains("Znalezione lokale sieci Cinema City"));
    assert!(list_output.contains("Warszawa - Janki"));
    assert!(list_output.contains("Wroclaw - Wroclavia"));

    let mut search_terminal = BufferTerminal::default();
    let search_exit_code = run_with_args(
        vec![
            "quickrep".to_string(),
            "venues".to_string(),
            "search".to_string(),
            "wroclaw".to_string(),
        ],
        &dependencies,
        &mut search_terminal,
    )
    .await;
    let search_output = search_terminal.into_string();
    assert_eq!(search_exit_code, 0);
    assert!(search_output.contains("Wroclaw - Wroclavia"));
    assert!(!search_output.contains("Warszawa - Janki"));
}

#[tokio::test]
async fn repertoire_command_exits_when_no_matching_venue_is_found() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(Vec::new(), Vec::new()),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string(), "nieistniejace".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 1);
    assert!(terminal.into_string().contains("Nie znaleziono żadnego lokalu o podanej nazwie."));
}

#[tokio::test]
async fn repertoire_command_exits_when_venue_name_is_ambiguous() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(Vec::new(), Vec::new()),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    DatabaseManager::new(dependencies.paths.db_file())
        .unwrap()
        .replace_venues(
            "cinema-city",
            &[
                CinemaVenue {
                    chain_id: "cinema-city".to_string(),
                    venue_name: "Warszawa - Arkadia".to_string(),
                    venue_id: "1".to_string(),
                },
                CinemaVenue {
                    chain_id: "cinema-city".to_string(),
                    venue_name: "Warszawa - Janki".to_string(),
                    venue_id: "2".to_string(),
                },
            ],
        )
        .unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string(), "warszawa".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 1);
    assert!(
        terminal
            .into_string()
            .contains("Podana nazwa lokalu jest niejednoznaczna. Znaleziono 2 pasujące wyniki.")
    );
}

#[tokio::test]
async fn repertoire_command_exits_when_default_venue_is_not_configured() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(Vec::new(), Vec::new()),
        FakeTmdbService { result: Default::default(), error: None },
    );
    let mut configured_settings = settings();
    configured_settings
        .user_preferences
        .default_venues
        .set(quick_repertoire::domain::CinemaChainId::CinemaCity, None);
    write_settings(&configured_settings, &dependencies.paths).unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "repertoire".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 1);
    assert!(
        terminal
            .into_string()
            .contains("Brak domyślnego lokalu skonfigurowanego dla sieci Cinema City.")
    );
}

#[tokio::test]
async fn configure_command_recovers_from_invalid_existing_config() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(
            vec!["cinema-city".to_string(), "dziś".to_string(), "Wroclaw - Wroclavia".to_string()],
            vec!["recovered-token".to_string()],
        ),
        FakeCinemaClient::new(
            Vec::new(),
            vec![CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    fs::write(dependencies.paths.config_file(), "[app\n").unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "configure".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 0);
    assert!(terminal.into_string().contains("Konfiguracja zapisana w config.ini."));
    assert!(
        fs::read_to_string(dependencies.paths.config_file())
            .unwrap()
            .contains("tmdb_access_token = recovered-token")
    );
}

#[tokio::test]
async fn configure_command_exits_with_explicit_message_when_runtime_dir_is_not_writable() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies_with_write_access_probe(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(Vec::new(), Vec::new()),
        FakeTmdbService { result: Default::default(), error: None },
        Box::new(FailingWriteAccessProbe {
            error_kind: io::ErrorKind::PermissionDenied,
            message: "access denied".to_string(),
        }),
    );
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["quickrep".to_string(), "configure".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 1);
    assert!(output.contains("Brak uprawnień do zapisu w katalogu aplikacji"));
    assert!(output.contains("Uruchom aplikację z podwyższonymi uprawnieniami"));
}

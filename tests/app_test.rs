mod support;

use std::collections::HashMap;
use std::fs;
use std::io;

use assert_cmd::Command;
use quick_repertoire::app::run_with_args;
use quick_repertoire::config::write_settings;
use quick_repertoire::domain::{CinemaVenue, MoviePlayDetails, Repertoire};
use quick_repertoire::error::AppError;
use quick_repertoire::output::BufferTerminal;
use quick_repertoire::persistence::DatabaseManager;
use support::{
    FailingWriteAccessProbe, FakeCinemaClient, FakePrompt, FakeTmdbService, dependencies,
    dependencies_with_write_access_probe, settings,
};
use tempfile::tempdir;

#[test]
fn binary_help_lists_top_level_commands() {
    Command::cargo_bin("app")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
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
            "app".to_string(),
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
                    play_times: vec!["10:00".to_string(), "12:30".to_string()],
                }],
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
        vec!["app".to_string(), "repertoire".to_string()],
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
                    play_times: vec!["10:00".to_string()],
                }],
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
        vec!["app".to_string(), "repertoire".to_string(), "wroclavia".to_string()],
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
async fn venues_update_updates_venues_correctly() {
    let temp_dir = tempdir().unwrap();
    let dependencies = dependencies(
        temp_dir.path(),
        FakePrompt::new(Vec::new(), Vec::new()),
        FakeCinemaClient::new(
            Vec::new(),
            vec![CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Test Venue".to_string(),
                venue_id: "9999".to_string(),
            }],
        ),
        FakeTmdbService { result: Default::default(), error: None },
    );
    write_settings(&settings(), &dependencies.paths).unwrap();
    let mut terminal = BufferTerminal::default();

    let exit_code = run_with_args(
        vec!["app".to_string(), "venues".to_string(), "update".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    assert_eq!(exit_code, 0);
    let output = terminal.into_string();
    assert!(output.contains("Aktualizowanie lokali dla sieci: Cinema City..."));
    assert!(output.contains("Lokale zaktualizowane w lokalnej bazie danych."));
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
}

#[tokio::test]
async fn configure_command_uses_existing_settings_when_available() {
    let temp_dir = tempdir().unwrap();
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
        vec!["app".to_string(), "configure".to_string()],
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
                    play_times: vec!["10:00".to_string()],
                }],
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
        vec!["app".to_string(), "repertoire".to_string(), "wroclavia".to_string()],
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
        vec!["app".to_string(), "configure".to_string()],
        &dependencies,
        &mut terminal,
    )
    .await;

    let output = terminal.into_string();
    assert_eq!(exit_code, 1);
    assert!(output.contains("Brak uprawnień do zapisu w katalogu aplikacji"));
    assert!(output.contains("Uruchom aplikację z podwyższonymi uprawnieniami"));
}

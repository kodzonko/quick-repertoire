use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use quick_repertoire::cinema::browser::ChromiumHtmlRenderer;
use quick_repertoire::cinema::common::normalize_lookup_text;
use quick_repertoire::cinema::registry::Registry;
use quick_repertoire::config::{AppPaths, Settings, write_settings};
use quick_repertoire::domain::CinemaChainId;
use quick_repertoire::persistence::DatabaseManager;
use tempfile::tempdir;

async fn measure_chain(chain_id: CinemaChainId, venue_names: &[&str]) {
    let runtime_dir = tempdir().unwrap();
    let binary = runtime_dir.path().join(format!("quickrep{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_quickrep"), &binary).unwrap();
    let paths = AppPaths::for_runtime_dir(runtime_dir.path().to_path_buf());
    let chain =
        Registry::new(Arc::new(ChromiumHtmlRenderer)).get_registered_chain(chain_id).unwrap();
    let started = Instant::now();
    let mut venues = chain.client.fetch_venues().await.expect("live venue discovery failed");
    println!("{} venue_discovery_ms={}", chain_id.as_str(), started.elapsed().as_millis());
    venues.sort_by(|left, right| left.venue_name.cmp(&right.venue_name));
    let database = DatabaseManager::new(paths.db_file()).unwrap();
    let token = std::env::var("TMDB_ACCESS_TOKEN").ok().filter(|value| !value.trim().is_empty());
    let mut tmdb_modes = vec![None];
    if let Some(token) = token {
        tmdb_modes.push(Some(token));
    }
    let mut settings = Settings::default();
    settings.user_preferences.default_chain = chain_id;
    settings.user_preferences.default_day = "jutro".into();

    for name in venue_names {
        let venue = venues
            .iter()
            .find(|venue| {
                normalize_lookup_text(&venue.venue_name).contains(&normalize_lookup_text(name))
            })
            .unwrap_or_else(|| panic!("{} venue missing: {name}", chain_id.as_str()));
        database.replace_venues(chain_id, std::slice::from_ref(venue)).unwrap();
        settings.user_preferences.default_venues.set(chain_id, Some(venue.venue_name.clone()));
        for access_token in &tmdb_modes {
            settings.user_preferences.tmdb_access_token = access_token.clone();
            write_settings(&settings, &paths).unwrap();
            for sample in 1..=2 {
                let started = Instant::now();
                let output = Command::new(&binary)
                    .arg("repertoire")
                    .env("RUST_LOG", "quick_repertoire::app=debug")
                    .output()
                    .expect("CLI should start");
                let elapsed = started.elapsed();
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(output.status.success(), "live CLI failed: {stdout}\n{stderr}");
                assert!(
                    !stdout.contains("Brak repertuaru do wyświetlenia"),
                    "empty repertoire: {stdout}"
                );
                assert!(stderr.contains("Repertoire timings"), "missing phase timings: {stderr}");
                if settings.user_preferences.tmdb_access_token.is_some() {
                    assert!(
                        !stdout.contains("Nie udało się pobrać danych z usługi TMDB"),
                        "TMDB enrichment failed"
                    );
                }
                println!(
                    "{} venue={:?} tmdb={} sample={sample} process_ms={}\n{}",
                    chain_id.as_str(),
                    venue.venue_name,
                    settings.user_preferences.tmdb_access_token.is_some(),
                    elapsed.as_millis(),
                    stderr.trim(),
                );
            }
        }
    }
}

#[tokio::test]
#[ignore = "makes live Cinema City requests and launches Chromium"]
async fn cinema_city_cold_process_performance() {
    measure_chain(CinemaChainId::CinemaCity, &["Manufaktura", "Wroclavia"]).await;
}

#[tokio::test]
#[ignore = "makes live Helios requests and launches Chromium"]
async fn helios_cold_process_performance() {
    measure_chain(CinemaChainId::Helios, &["Łódź", "Wrocław"]).await;
}

#[tokio::test]
#[ignore = "makes live Multikino requests"]
async fn multikino_cold_process_performance() {
    measure_chain(CinemaChainId::Multikino, &["Złote Tarasy", "Poznań"]).await;
}

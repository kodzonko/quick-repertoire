mod support;

use cinema_repertoire_analyzer::domain::CinemaVenue;
use cinema_repertoire_analyzer::persistence::DatabaseManager;
use tempfile::tempdir;

#[test]
fn database_manager_bootstraps_schema_on_init() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_db.sqlite");

    let db_manager = DatabaseManager::new(db_path.clone()).unwrap();

    assert!(db_path.exists());
    assert!(db_manager.get_all_venues("cinema-city").unwrap().is_empty());
}

#[test]
fn replace_venues_replaces_existing_records_only_for_selected_chain() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().join("test_db.sqlite")).unwrap();

    db_manager
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Warszawa - Janki".to_string(),
                venue_id: "1".to_string(),
            }],
        )
        .unwrap();
    db_manager
        .replace_venues(
            "helios",
            &[CinemaVenue {
                chain_id: "helios".to_string(),
                venue_name: "Lodz - Sukcesja".to_string(),
                venue_id: "2".to_string(),
            }],
        )
        .unwrap();
    db_manager
        .replace_venues(
            "cinema-city",
            &[CinemaVenue {
                chain_id: "cinema-city".to_string(),
                venue_name: "Wroclaw - Wroclavia".to_string(),
                venue_id: "3".to_string(),
            }],
        )
        .unwrap();

    assert_eq!(
        db_manager
            .get_all_venues("cinema-city")
            .unwrap()
            .into_iter()
            .map(|venue| venue.venue_name)
            .collect::<Vec<_>>(),
        vec!["Wroclaw - Wroclavia".to_string()]
    );
    assert_eq!(
        db_manager
            .get_all_venues("helios")
            .unwrap()
            .into_iter()
            .map(|venue| venue.venue_name)
            .collect::<Vec<_>>(),
        vec!["Lodz - Sukcesja".to_string()]
    );
}

#[test]
fn find_venues_by_name_returns_matching_venues_for_selected_chain() {
    let temp_dir = tempdir().unwrap();
    let db_manager = DatabaseManager::new(temp_dir.path().join("test_db.sqlite")).unwrap();

    db_manager
        .replace_venues(
            "cinema-city",
            &[
                CinemaVenue {
                    chain_id: "cinema-city".to_string(),
                    venue_name: "Warszawa - Janki".to_string(),
                    venue_id: "1".to_string(),
                },
                CinemaVenue {
                    chain_id: "cinema-city".to_string(),
                    venue_name: "Warszawa - Arkadia".to_string(),
                    venue_id: "2".to_string(),
                },
                CinemaVenue {
                    chain_id: "cinema-city".to_string(),
                    venue_name: "Wroclaw - Wroclavia".to_string(),
                    venue_id: "3".to_string(),
                },
            ],
        )
        .unwrap();
    db_manager
        .replace_venues(
            "helios",
            &[CinemaVenue {
                chain_id: "helios".to_string(),
                venue_name: "Warszawa - Blue City".to_string(),
                venue_id: "4".to_string(),
            }],
        )
        .unwrap();

    assert_eq!(
        db_manager
            .find_venues_by_name("cinema-city", "Warszawa")
            .unwrap()
            .into_iter()
            .map(|venue| venue.venue_name)
            .collect::<Vec<_>>(),
        vec!["Warszawa - Arkadia".to_string(), "Warszawa - Janki".to_string()]
    );
}

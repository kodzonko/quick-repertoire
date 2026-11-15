use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use rusqlite::{Connection, OptionalExtension, params};

use crate::config::{DefaultVenues, Settings, UserPreferences};
use crate::domain::{CinemaChainId, CinemaVenue};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_file_path: PathBuf,
}

impl DatabaseManager {
    pub fn new(db_file_path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = db_file_path.parent() {
            fs::create_dir_all(parent).map_err(|_| AppError::database_connection(&db_file_path))?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(unix)]
        options.mode(0o600);
        options.open(&db_file_path).map_err(|_| AppError::database_connection(&db_file_path))?;
        #[cfg(unix)]
        fs::set_permissions(&db_file_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| AppError::database_connection(&db_file_path))?;
        let manager = Self { db_file_path };
        manager.bootstrap_schema()?;
        Ok(manager)
    }

    pub fn load_settings(&self) -> AppResult<Option<Settings>> {
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let preferences = transaction
            .query_row(
                "SELECT default_chain, default_day, tmdb_access_token FROM user_preferences WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?)),
            )
            .optional()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let Some((chain, default_day, tmdb_access_token)) = preferences else {
            return Ok(None);
        };
        let default_chain = CinemaChainId::from_value(&chain).map_err(|_| {
            AppError::configuration(
                "Nieprawidłowa sieć w ustawieniach. Uruchom `quickrep configure`.",
            )
        })?;
        let mut statement = transaction
            .prepare("SELECT chain_id, venue_name FROM default_venues")
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let mut default_venues = DefaultVenues::default();
        for row in rows {
            let (chain, venue) =
                row.map_err(|_| AppError::database_connection(&self.db_file_path))?;
            let chain = CinemaChainId::from_value(&chain).map_err(|_| {
                AppError::configuration(
                    "Nieprawidłowa sieć w ustawieniach. Uruchom `quickrep configure`.",
                )
            })?;
            default_venues.set(chain, Some(venue));
        }
        Ok(Some(Settings {
            user_preferences: UserPreferences {
                default_chain,
                default_day,
                tmdb_access_token,
                default_venues,
            },
        }))
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.save_configuration(settings, &HashMap::new())
    }

    pub fn save_configuration(
        &self,
        settings: &Settings,
        venues_by_chain: &HashMap<CinemaChainId, Vec<CinemaVenue>>,
    ) -> AppResult<()> {
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        self.write_preferences(&transaction, settings)?;
        self.write_venues(&transaction, venues_by_chain)?;
        transaction.commit().map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    fn write_preferences(&self, connection: &Connection, settings: &Settings) -> AppResult<()> {
        let preferences = &settings.user_preferences;
        connection
            .execute(
                "INSERT INTO user_preferences (id, default_chain, default_day, tmdb_access_token)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET default_chain = excluded.default_chain,
                     default_day = excluded.default_day, tmdb_access_token = excluded.tmdb_access_token",
                params![preferences.default_chain.as_str(), preferences.default_day, preferences.tmdb_access_token],
            )
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        connection
            .execute("DELETE FROM default_venues", [])
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        for chain in CinemaChainId::all() {
            if let Some(venue) = preferences.default_venues.get(*chain) {
                connection
                    .execute(
                        "INSERT INTO default_venues (chain_id, venue_name) VALUES (?1, ?2)",
                        params![chain.as_str(), venue],
                    )
                    .map_err(|_| AppError::database_connection(&self.db_file_path))?;
            }
        }
        Ok(())
    }

    pub fn has_venues(&self) -> AppResult<bool> {
        self.open_connection()?
            .query_row("SELECT EXISTS(SELECT 1 FROM cinema_venues)", [], |row| row.get(0))
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    pub fn get_all_venues(&self, chain_id: impl AsRef<str>) -> AppResult<Vec<CinemaVenue>> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "SELECT chain_id, venue_id, venue_name \
                 FROM cinema_venues \
                 WHERE chain_id = ?1 \
                 ORDER BY venue_name",
            )
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let rows = statement
            .query_map([chain_id.as_ref()], |row| {
                Ok(CinemaVenue {
                    chain_id: row.get(0)?,
                    venue_id: row.get(1)?,
                    venue_name: row.get(2)?,
                })
            })
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    pub fn replace_venues(&self, chain_id: CinemaChainId, venues: &[CinemaVenue]) -> AppResult<()> {
        self.replace_venues_batch(&HashMap::from([(chain_id, venues.to_vec())]))
    }

    pub fn replace_venues_batch(
        &self,
        venues_by_chain: &HashMap<CinemaChainId, Vec<CinemaVenue>>,
    ) -> AppResult<()> {
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        self.write_venues(&transaction, venues_by_chain)?;
        transaction.commit().map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    fn write_venues(
        &self,
        connection: &Connection,
        venues_by_chain: &HashMap<CinemaChainId, Vec<CinemaVenue>>,
    ) -> AppResult<()> {
        for (chain_id, venues) in venues_by_chain {
            connection
                .execute("DELETE FROM cinema_venues WHERE chain_id = ?1", [chain_id.as_str()])
                .map_err(|_| AppError::database_connection(&self.db_file_path))?;
            for venue in venues {
                connection
                    .execute(
                        "INSERT INTO cinema_venues (chain_id, venue_id, venue_name) VALUES (?1, ?2, ?3)",
                        params![venue.chain_id, venue.venue_id, venue.venue_name],
                    )
                    .map_err(|_| AppError::database_connection(&self.db_file_path))?;
            }
        }
        Ok(())
    }

    pub fn find_venues_by_name(
        &self,
        chain_id: impl AsRef<str>,
        search_string: &str,
    ) -> AppResult<Vec<CinemaVenue>> {
        let pattern = if search_string.contains('%') {
            search_string.to_string()
        } else {
            format!("%{search_string}%")
        };
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "SELECT chain_id, venue_id, venue_name \
                 FROM cinema_venues \
                 WHERE chain_id = ?1 AND venue_name LIKE ?2 COLLATE NOCASE \
                 ORDER BY venue_name",
            )
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        let rows = statement
            .query_map(params![chain_id.as_ref(), pattern], |row| {
                Ok(CinemaVenue {
                    chain_id: row.get(0)?,
                    venue_id: row.get(1)?,
                    venue_name: row.get(2)?,
                })
            })
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    fn bootstrap_schema(&self) -> AppResult<()> {
        let connection = self.open_connection()?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS cinema_venues (
                    chain_id TEXT NOT NULL,
                    venue_id TEXT NOT NULL,
                    venue_name TEXT NOT NULL,
                    PRIMARY KEY (chain_id, venue_id)
                );
                CREATE TABLE IF NOT EXISTS user_preferences (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    default_chain TEXT NOT NULL,
                    default_day TEXT NOT NULL,
                    tmdb_access_token TEXT
                );
                CREATE TABLE IF NOT EXISTS default_venues (
                    chain_id TEXT PRIMARY KEY NOT NULL,
                    venue_name TEXT NOT NULL
                );",
            )
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    fn open_connection(&self) -> AppResult<Connection> {
        Connection::open(&self.db_file_path)
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }
}

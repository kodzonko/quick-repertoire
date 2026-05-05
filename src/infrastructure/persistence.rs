use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{Connection, params};

use crate::domain::CinemaVenue;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_file_path: PathBuf,
}

impl DatabaseManager {
    pub fn new(db_file_path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = db_file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| AppError::database_connection(&db_file_path))?;
        }
        let manager = Self { db_file_path };
        manager.bootstrap_schema()?;
        Ok(manager)
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

    pub fn replace_venues(
        &self,
        chain_id: impl AsRef<str>,
        venues: &[CinemaVenue],
    ) -> AppResult<()> {
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        transaction
            .execute("DELETE FROM cinema_venues WHERE chain_id = ?1", [chain_id.as_ref()])
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        for venue in venues {
            transaction
                .execute(
                    "INSERT INTO cinema_venues (chain_id, venue_id, venue_name) VALUES (?1, ?2, ?3)",
                    params![venue.chain_id, venue.venue_id, venue.venue_name],
                )
                .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        }
        transaction.commit().map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    pub fn replace_venues_batch(
        &self,
        venues_by_chain: &HashMap<String, Vec<CinemaVenue>>,
    ) -> AppResult<()> {
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::database_connection(&self.db_file_path))?;
        for (chain_id, venues) in venues_by_chain {
            transaction
                .execute("DELETE FROM cinema_venues WHERE chain_id = ?1", [chain_id.as_str()])
                .map_err(|_| AppError::database_connection(&self.db_file_path))?;
            for venue in venues {
                transaction
                    .execute(
                        "INSERT INTO cinema_venues (chain_id, venue_id, venue_name) VALUES (?1, ?2, ?3)",
                        params![venue.chain_id, venue.venue_id, venue.venue_name],
                    )
                    .map_err(|_| AppError::database_connection(&self.db_file_path))?;
            }
        }
        transaction.commit().map_err(|_| AppError::database_connection(&self.db_file_path))
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
                );",
            )
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }

    fn open_connection(&self) -> AppResult<Connection> {
        Connection::open(&self.db_file_path)
            .map_err(|_| AppError::database_connection(&self.db_file_path))
    }
}

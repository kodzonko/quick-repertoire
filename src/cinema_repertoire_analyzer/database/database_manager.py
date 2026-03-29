"""Module containing SQLite connection and operations wrappers.

In general, functions in this class will call the database directly.
"""

from pathlib import Path
from sqlite3 import Error

import sqlalchemy
import sqlalchemy.exc
import sqlalchemy.orm
from loguru import logger

from cinema_repertoire_analyzer.database.models import Base, CinemaVenues
from cinema_repertoire_analyzer.exceptions import DatabaseConnectionError


class DatabaseManager:
    """Class responsible for connecting to the database and executing queries."""

    def __init__(self, db_file_path: Path | str) -> None:
        self._db_file_path = Path(db_file_path)
        sqlite_uri = f"sqlite:///{self._db_file_path}"
        try:
            self._db_file_path.parent.mkdir(parents=True, exist_ok=True)
            engine = sqlalchemy.create_engine(sqlite_uri)
            self._engine = engine
            Base.metadata.create_all(engine)
            self._session_constructor = sqlalchemy.orm.sessionmaker(engine)
            logger.debug(f"Connection to the database {sqlite_uri} successful.")
        except (Error, OSError, sqlalchemy.exc.SQLAlchemyError) as e:
            raise DatabaseConnectionError(
                f"Nie udało się połączyć z bazą danych {sqlite_uri}. Spróbuj jeszcze raz."
            ) from e

    def close(self) -> None:
        """Dispose the underlying SQLAlchemy engine."""
        self._engine.dispose()

    def get_all_venues(self) -> list[CinemaVenues]:
        """Get all venues for a specified cinema chain from the database."""
        with self._session_constructor() as session:
            results = session.query(CinemaVenues).all()
            return results

    def update_cinema_venues(self, venues: list[CinemaVenues]) -> None:
        """Update cinema venues in the database.

        Function will remove all records from the table and insert new ones.
        """
        with self._session_constructor() as session:
            session.query(CinemaVenues).delete()
            session.add_all(venues)
            session.commit()

    def find_venues_by_name(self, search_string: str) -> list[CinemaVenues]:
        """Find a venue of a specified cinema chain by name.

        Conducts a permissive search
        """
        with self._session_constructor() as session:
            return (
                session.query(CinemaVenues)
                .filter(CinemaVenues.venue_name.ilike(f"%{search_string}%"))
                .all()
            )

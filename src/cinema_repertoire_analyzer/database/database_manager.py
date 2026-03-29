"""Module containing SQLite connection and operations wrappers.

In general, functions in this class will call the database directly.
"""

from pathlib import Path
from sqlite3 import Error

import sqlalchemy
import sqlalchemy.exc
import sqlalchemy.orm
from loguru import logger

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue
from cinema_repertoire_analyzer.database.models import Base, CinemaVenueRecord
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

    def get_all_venues(self, chain_id: str | CinemaChainId) -> list[CinemaVenue]:
        """Get all cached venues for a specified cinema chain."""
        chain_id_value = chain_id.value if isinstance(chain_id, CinemaChainId) else chain_id
        with self._session_constructor() as session:
            results = (
                session.query(CinemaVenueRecord)
                .filter(CinemaVenueRecord.chain_id == chain_id_value)
                .order_by(CinemaVenueRecord.venue_name)
                .all()
            )
            return [result.to_domain() for result in results]

    def replace_venues(self, chain_id: str | CinemaChainId, venues: list[CinemaVenue]) -> None:
        """Replace cached venues for a specified cinema chain.

        Function will remove all records from the table and insert new ones.
        """
        chain_id_value = chain_id.value if isinstance(chain_id, CinemaChainId) else chain_id
        with self._session_constructor() as session:
            (
                session.query(CinemaVenueRecord)
                .filter(CinemaVenueRecord.chain_id == chain_id_value)
                .delete()
            )
            session.add_all(CinemaVenueRecord.from_domain(venue) for venue in venues)
            session.commit()

    def find_venues_by_name(
        self, chain_id: str | CinemaChainId, search_string: str
    ) -> list[CinemaVenue]:
        """Find venues for a specified cinema chain by name.

        Conducts a permissive search
        """
        chain_id_value = chain_id.value if isinstance(chain_id, CinemaChainId) else chain_id
        with self._session_constructor() as session:
            results = (
                session.query(CinemaVenueRecord)
                .filter(CinemaVenueRecord.chain_id == chain_id_value)
                .filter(CinemaVenueRecord.venue_name.ilike(f"%{search_string}%"))
                .order_by(CinemaVenueRecord.venue_name)
                .all()
            )
            return [result.to_domain() for result in results]

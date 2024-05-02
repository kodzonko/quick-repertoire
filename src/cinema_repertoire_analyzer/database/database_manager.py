"""Module containing SQLite connection and operations wrappers.

In general, functions in this class will call the database directly.
"""

from os import PathLike
from sqlite3 import Error

import sqlalchemy
from loguru import logger

from cinema_repertoire_analyzer.database.db_utils import get_table_by_cinema_chain
from cinema_repertoire_analyzer.database.models import CinemaVenuesBase
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.exceptions import DBConnectionError


class DatabaseManager:
    """Class responsible for connecting to the database and executing queries."""

    def __init__(self, db_file_path: PathLike) -> None:
        try:
            engine = sqlalchemy.create_engine(f"sqlite:///{db_file_path}")
            self._session_constructor = sqlalchemy.orm.sessionmaker(engine)
            logger.info("Connection to the database successful.")
        except Error as e:
            logger.error(f"Unable to connect with the database: {e}")
            raise DBConnectionError("Failed to connect with the database.")

    def get_cinema_venues(self, cinema_chain: CinemaChain) -> list[CinemaVenuesBase]:
        """Get all cinema venues for a specified cinema chain from the database."""
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            results = session.query(table).all()
            return results

    def update_cinema_venues(
        self, cinema_chain: CinemaChain, venues: list[CinemaVenuesBase]
    ) -> None:
        """Update cinema venues in the database.

        Function will remove all records from the table and insert new ones.
        """
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            session.query(table).delete()
            session.add_all(venues)
            session.commit()

    def get_venue_by_venue_name(
        self, cinema_chain: CinemaChain, venue_name: str
    ) -> CinemaVenuesBase:
        """Get venue by name."""
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            return session.query(table).filter_by(venue_name=venue_name).one()

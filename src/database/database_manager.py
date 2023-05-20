"""Module containing SQLite connection and operations wrappers.

In general, functions in this class will call the database directly.
"""
import datetime
from pathlib import Path
from sqlite3 import Error

import sqlalchemy
from loguru import logger

import settings
from database.models import CinemaVenues, Repertoire
from enums import CinemaChain
from exceptions import DBConnectionError


class DatabaseManager:
    """Class responsible for connecting to the database and executing queries."""

    def __init__(self, db_file_path: Path | str = settings.DB_FILE_PATH) -> None:
        try:
            engine = sqlalchemy.create_engine(f"sqlite:///{db_file_path}")
            self._session_constructor = sqlalchemy.orm.sessionmaker(engine)
            logger.info("Connection to the database successful.")
        except Error as e:
            logger.error("Unable to connect with the database: %s.", e)
            raise DBConnectionError("Failed to connect with the database.")

    def get_cinema_venues(
        self, cinema: CinemaChain, city: str | None = None
    ) -> list[str]:
        """Get all cinema venues for a specified cinema chain from the database.

        Optionally filter by city.
        """
        queries = [CinemaVenues.cinema_chain == str(cinema)]
        if city:
            queries.append(CinemaVenues.city == city)

        with self._session_constructor() as session:
            results = (
                session.query(CinemaVenues.venue_name)
                .filter(*queries)
                .group_by(CinemaVenues.city)
                .all()
            )
            return [item[0] for item in results]

    def update_cinema_venues(self, venues: list[CinemaVenues]) -> None:
        """Update cinema venues in the database.

        Function will remove all records from the table and insert new ones.
        """
        with self._session_constructor() as session:
            session.query(CinemaVenues).delete()
            session.add_all(venues)

    def get_repertoire(
        self,
        date: datetime.date,
        cinema: CinemaChain,
        *,
        venue: str | None = None,
        city: str | None = None,
        format: str | None = None,
        language: str | None = None,
    ) -> list[Repertoire]:
        """Get repertoire matching specified date and cinema."""
        with self._session_constructor() as session:
            clauses = [
                Repertoire.play_time == date,
                CinemaVenues.cinema_chain == str(cinema),
            ]
            if venue:
                clauses.append(CinemaVenues.venue_name == venue)
            if city:
                clauses.append(CinemaVenues.city == city)
            if format:
                clauses.append(Repertoire.movie_format == format)
            if language:
                clauses.append(Repertoire.movie_language == language)
            return session.query(Repertoire).filter(*clauses).all()

    def get_venue_by_venue_id(self, venue_id: int) -> CinemaVenues:
        """Get venue by id."""
        with self._session_constructor() as session:
            return session.query(CinemaVenues).filter_by(venue_id=venue_id).one()

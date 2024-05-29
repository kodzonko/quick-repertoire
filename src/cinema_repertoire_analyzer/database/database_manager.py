"""Module containing SQLite connection and operations wrappers.

In general, functions in this class will call the database directly.
"""

from pathlib import Path
from sqlite3 import Error

import sqlalchemy
import typer
from loguru import logger

from cinema_repertoire_analyzer.database.db_utils import get_table_by_cinema_chain
from cinema_repertoire_analyzer.database.models import VenueData
from cinema_repertoire_analyzer.enums import CinemaChain


class DatabaseManager:
    """Class responsible for connecting to the database and executing queries."""

    def __init__(self, db_file_path: Path | str) -> None:
        db_open_path = f"sqlite:///{db_file_path}"
        try:
            engine = sqlalchemy.create_engine(db_open_path)
            self._session_constructor = sqlalchemy.orm.sessionmaker(engine)
            logger.debug(f"Connection to the database {db_open_path} successful.")
        except Error as e:
            typer.echo(f"Nie udało się połączyć z bazą danych {db_open_path}. Spróbuj jeszcze raz.")
            raise typer.Exit(code=1) from e

    def get_all_venues(self, cinema_chain: CinemaChain) -> list[VenueData]:
        """Get all venues for a specified cinema chain from the database."""
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            results = session.query(table).all()
            return results

    def update_cinema_venues(self, cinema_chain: CinemaChain, venues: list[VenueData]) -> None:
        """Update cinema venues in the database.

        Function will remove all records from the table and insert new ones.
        """
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            session.query(table).delete()
            session.add_all(venues)
            session.commit()

    def find_venues_by_name(
        self, cinema_chain: CinemaChain, search_string: str
    ) -> VenueData | list[VenueData]:
        """Find a venue of a specified cinema chain by name.

        Conducts a permissive search

        Raises:
        typer.Exit: If no venue is found.
        """
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            results = (
                session.query(table).filter(table.venue_name.ilike(f"%{search_string}%")).all()
            )
            if len(results) == 1:
                return results[0]
            elif len(results) == 0:
                typer.echo("Nie znaleziono żadnego lokalu o podanej nazwie.")
                raise typer.Exit(code=1)
            return results

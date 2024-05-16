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
        try:
            engine = sqlalchemy.create_engine(f"sqlite:///{db_file_path}")
            self._session_constructor = sqlalchemy.orm.sessionmaker(engine)
            logger.debug("Connection to the database successful.")
        except Error as e:
            typer.echo("Nie udało się połączyć z lokalną bazą danych. Spróbuj jeszcze raz.")
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

    def find_venue_by_name(self, cinema_chain: CinemaChain, search_string: str) -> VenueData:
        """Find a venue of a specified cinema chain by name.

        Conducts a permissive search

        Raises:
        typer.Exit: If no venue is found or if the venue name is ambiguous.
        """
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            # return session.query(table).filter_by(venue_name=venue_name).one()
            results = session.query(table).filter(table.venue_name.ilike(search_string)).all()
            if len(results) == 1:
                return results[0]
            elif len(results) == 0:
                typer.echo("Nie znaleziono lokalu o podanej nazwie.")
                raise typer.Exit(code=1)
            typer.echo(
                f"Nazwa lokalu podana przez użytkownika jest niejednoznaczna. Znaleziono "
                f"{len(results)} {"pasujące wyniki" if len(results) < 5 else "pasujących wyników"}."
            )
            raise typer.Exit(code=1)

    def search_venues_by_name(
        self, cinema_chain: CinemaChain, search_string: str
    ) -> list[VenueData]:
        """Search for cinema venues by name."""
        table = get_table_by_cinema_chain(cinema_chain)
        with self._session_constructor() as session:
            results = session.query(table).filter(table.venue_name.like(search_string)).all()
            return results

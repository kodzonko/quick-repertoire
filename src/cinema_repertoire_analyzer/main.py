from typing import Annotated

import typer
from rich.console import Console

from cinema_repertoire_analyzer.cinema_api.cinema_utils import cinema_factory
from cinema_repertoire_analyzer.cli_utils import (
    cinema_input_parser,
    cinema_venue_input_parser,
    date_input_parser,
    db_venues_to_cli,
)
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.settings import Settings, get_settings


def make_app(settings: Settings = get_settings()) -> typer.Typer:
    app = typer.Typer()
    db_manager = DatabaseManager(settings.user_preferences.db_file_path)
    console = Console()

    @app.command()
    def repertoire(
        cinema: Annotated[
            str, typer.Argument()
        ] = settings.user_preferences.default_cinema,
        venue: Annotated[
            str, typer.Argument()
        ] = settings.user_preferences.default_cinema_venue,
        date: Annotated[str, typer.Argument()] = settings.user_preferences.default_day,
    ):
        cinema = cinema_input_parser(cinema)
        cinema_venue_input_parser(venue)
        date = date_input_parser(date)
        print(date)

    @app.command()
    def list_venues(
        cinema: Annotated[
            str, typer.Argument()
        ] = settings.user_preferences.default_cinema,
    ):
        cinema_chain = cinema_input_parser(cinema)
        venues = db_manager.get_cinema_venues(cinema_chain)
        db_venues_to_cli(venues, console)

    @app.command()
    def update_venues(
        cinema_name: Annotated[
            str, typer.Argument()
        ] = settings.user_preferences.default_cinema,
    ):
        cinema_chain = cinema_input_parser(cinema_name)
        print(f"Updating venues for {cinema_chain.value}...")
        cinema = cinema_factory(cinema_chain, settings)
        venues = cinema.fetch_cinema_venues_list()
        db_manager.update_cinema_venues(cinema_chain, venues)
        print("Venues updated in the local database.")

    return app


if __name__ == "__main__":
    make_app()

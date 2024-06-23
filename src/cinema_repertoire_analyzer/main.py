import builtins
from typing import Annotated

import rich
import typer

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import RepertoireCliTableMetadata
from cinema_repertoire_analyzer.cli_utils import (
    cinema_venue_input_parser,
    date_input_parser,
    db_venues_to_cli,
    repertoire_to_cli,
)
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.ratings_api.tmdb import (
    get_movie_ratings_and_summaries,
    verify_api_key,
)
from cinema_repertoire_analyzer.settings import Settings, get_settings


def make_app(settings: Settings = get_settings()) -> typer.Typer:
    """Create the Typer application."""
    venues_app = typer.Typer()
    app = typer.Typer()
    app.add_typer(venues_app, name="venues")
    db_manager = DatabaseManager(settings.DB_FILE)
    console = rich.console.Console()

    @app.command()
    def repertoire(
        venue_name: Annotated[
            str, typer.Argument()
        ] = settings.USER_PREFERENCES.DEFAULT_CINEMA_VENUE,
        date: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_DAY,
    ) -> None:
        venue_name_parsed = cinema_venue_input_parser(venue_name)
        venue = db_manager.find_venues_by_name(venue_name_parsed)
        if isinstance(venue, builtins.list):
            typer.echo(
                f"Podana nazwa lokalu jest niejednoznaczna. Znaleziono "
                f"{len(venue)} {"pasujące wyniki" if len(venue) < 5 else "pasujących wyników"}."
            )
            raise typer.Exit(code=1)

        date_parsed: str = date_input_parser(date)

        cinema_instance = CinemaCity(
            settings.CINEMA_CITY_SETTINGS.REPERTOIRE_URL,
            settings.CINEMA_CITY_SETTINGS.VENUES_LIST_URL,
        )
        fetched_repertoire = cinema_instance.fetch_repertoire(date_parsed, venue)
        tmdb_enabled = verify_api_key(settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN)
        ratings = {}
        if tmdb_enabled and settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN:
            movie_titles = [repertoire.title for repertoire in fetched_repertoire]
            ratings = get_movie_ratings_and_summaries(
                movie_titles, settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN
            )
        else:
            console.print(
                "Klucz API do usługi TMDB nie jest skonfigurowany. "
                "Niektóre funkcje mogą być niedostępne.",
                style="bold red",
            )

        table_metadata = RepertoireCliTableMetadata(
            repertoire_date=date_parsed,
            cinema_venue_name=venue.venue_name,  # type: ignore
        )
        repertoire_to_cli(fetched_repertoire, table_metadata, ratings, console)

    @venues_app.command()
    def list() -> None:
        venues = db_manager.get_all_venues()
        db_venues_to_cli(venues, console)

    @venues_app.command()
    def update() -> None:
        typer.echo("Aktualizowanie lokali dla kina: Cinema City...")
        cinema_instance = CinemaCity(
            settings.CINEMA_CITY_SETTINGS.REPERTOIRE_URL,
            settings.CINEMA_CITY_SETTINGS.VENUES_LIST_URL,
        )
        venues = cinema_instance.fetch_cinema_venues_list()
        db_manager.update_cinema_venues(venues)
        typer.echo("Lokale zaktualizowane w lokalnej bazie danych.")

    @venues_app.command()
    def search(venue_name: Annotated[str, typer.Argument()] = ""):
        found_venues = db_manager.find_venues_by_name(venue_name)
        db_venues_to_cli(found_venues, console)

    return app


if __name__ == "__main__":
    make_app()()

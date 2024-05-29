from typing import Annotated, List

import rich
import typer

from cinema_repertoire_analyzer.cinema_api.cinema_utils import cinema_factory
from cinema_repertoire_analyzer.cinema_api.models import RepertoireCliTableMetadata
from cinema_repertoire_analyzer.cli_utils import (
    cinema_input_parser,
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
        cinema_chain: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_CINEMA,
        venue_name: Annotated[
            str, typer.Argument()
        ] = settings.USER_PREFERENCES.DEFAULT_CINEMA_VENUE,
        date: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_DAY,
    ) -> None:
        cinema_chain = cinema_input_parser(cinema_chain)
        venue_name_parsed = cinema_venue_input_parser(venue_name)
        venue = db_manager.find_venues_by_name(cinema_chain, venue_name_parsed)
        if isinstance(venue, List):
            typer.echo(
                f"Podana nazwa lokalu jest niejednoznaczna. Znaleziono "
                f"{len(venue)} {"pasujące wyniki" if len(venue) < 5 else "pasujących wyników"}."
            )
            raise typer.Exit(code=1)

        date_parsed: str = date_input_parser(date)

        cinema_instance = cinema_factory(cinema_chain, settings)
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

        # noinspection PyTypeChecker
        table_metadata = RepertoireCliTableMetadata(
            repertoire_date=date_parsed,
            cinema_chain_name=cinema_chain.value,
            cinema_venue_name=venue.venue_name,  # type: ignore
        )
        repertoire_to_cli(fetched_repertoire, table_metadata, ratings, console)

    @venues_app.command()
    def list(
        cinema: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_CINEMA,
    ) -> None:
        cinema_chain = cinema_input_parser(cinema)
        venues = db_manager.get_all_venues(cinema_chain)
        db_venues_to_cli(venues, console)

    @venues_app.command()
    def update(
        cinema_name: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_CINEMA,
    ) -> None:
        cinema_chain = cinema_input_parser(cinema_name)
        typer.echo(f"Aktualizowanie lokali dla kina: {cinema_chain.value}...")
        cinema = cinema_factory(cinema_chain, settings)
        venues = cinema.fetch_cinema_venues_list()
        db_manager.update_cinema_venues(cinema_chain, venues)
        typer.echo("Lokale zaktualizowane w lokalnej bazie danych.")

    @venues_app.command()
    def search(
        cinema_name: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_CINEMA,
        venue_name: Annotated[str, typer.Argument()] = "",
    ):
        cinema_chain = cinema_input_parser(cinema_name)
        found_venues = db_manager.find_venues_by_name(cinema_chain, venue_name)
        db_venues_to_cli(found_venues, console)

    return app


if __name__ == "__main__":
    make_app()()

from pathlib import Path
from typing import Annotated

import anyio
import httpx
import typer
from rich.console import Console

from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.models import RepertoireCliTableMetadata
from cinema_repertoire_analyzer.cli_utils import (
    cinema_venue_input_parser,
    date_input_parser,
    db_venues_to_cli,
    repertoire_to_cli,
)
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.database.models import CinemaVenues
from cinema_repertoire_analyzer.exceptions import (
    AmbiguousVenueMatchError,
    AppError,
    VenueNotFoundError,
)
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails
from cinema_repertoire_analyzer.ratings_api.tmdb import get_movie_ratings_and_summaries
from cinema_repertoire_analyzer.settings import Settings, get_settings


def _resolve_single_venue(found_venues: list[CinemaVenues]) -> CinemaVenues:
    """Resolve a venue search result to exactly one venue."""
    if not found_venues:
        raise VenueNotFoundError("Nie znaleziono żadnego lokalu o podanej nazwie.")
    if len(found_venues) > 1:
        raise AmbiguousVenueMatchError(len(found_venues))
    return found_venues[0]


def _handle_cli_error(error: AppError) -> None:
    """Display a user-facing error and exit the CLI."""
    typer.echo(str(error))
    raise typer.Exit(code=1) from error


def _build_database_manager(db_file_path: Path | str) -> DatabaseManager:
    """Create a database manager and map app errors to CLI output."""
    try:
        return DatabaseManager(db_file_path)
    except AppError as error:
        _handle_cli_error(error)
    raise AssertionError("unreachable")


def _warn_tmdb_unavailable(console: Console, message: str) -> None:
    """Display a warning when TMDB data cannot be loaded."""
    console.print(message, style="bold red")


def _load_tmdb_ratings(
    movie_titles: list[str], access_token: str | None, console: Console
) -> dict[str, TmdbMovieDetails]:
    """Fetch TMDB ratings when a token is configured and the service is reachable."""
    if not movie_titles:
        return {}
    if not access_token:
        _warn_tmdb_unavailable(
            console,
            "Klucz API do usługi TMDB nie jest skonfigurowany. "
            "Niektóre funkcje mogą być niedostępne.",
        )
        return {}
    try:
        return get_movie_ratings_and_summaries(movie_titles, access_token)
    except httpx.HTTPError:
        _warn_tmdb_unavailable(
            console,
            "Nie udało się pobrać danych z usługi TMDB. Niektóre funkcje mogą być niedostępne.",
        )
        return {}


def make_app(settings: Settings | None = None) -> typer.Typer:
    """Create the Typer application."""
    if settings is None:
        settings = get_settings()

    venues_app = typer.Typer()
    app = typer.Typer()
    app.add_typer(venues_app, name="venues")
    db_manager = _build_database_manager(settings.DB_FILE)
    setattr(app, "_db_manager", db_manager)
    console = Console()

    @app.command()
    def repertoire(
        venue_name: Annotated[
            str, typer.Argument()
        ] = settings.USER_PREFERENCES.DEFAULT_CINEMA_VENUE,
        date: Annotated[str, typer.Argument()] = settings.USER_PREFERENCES.DEFAULT_DAY,
    ) -> None:
        try:
            venue_name_parsed = cinema_venue_input_parser(venue_name)
            found_venues = db_manager.find_venues_by_name(venue_name_parsed)
            venue = _resolve_single_venue(found_venues)
            date_parsed: str = date_input_parser(date)

            cinema_instance = CinemaCity(
                settings.CINEMA_CITY_SETTINGS.REPERTOIRE_URL,
                settings.CINEMA_CITY_SETTINGS.VENUES_LIST_URL,
            )
            fetched_repertoire = anyio.run(cinema_instance.fetch_repertoire, date_parsed, venue)
            movie_titles = [repertoire.title for repertoire in fetched_repertoire]
            ratings = _load_tmdb_ratings(
                movie_titles, settings.USER_PREFERENCES.TMDB_ACCESS_TOKEN, console
            )

            table_metadata = RepertoireCliTableMetadata(
                repertoire_date=date_parsed, cinema_venue_name=str(venue.venue_name)
            )
            repertoire_to_cli(fetched_repertoire, table_metadata, ratings, console)
        except AppError as error:
            _handle_cli_error(error)

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
        venues = anyio.run(cinema_instance.fetch_cinema_venues_list)
        db_manager.update_cinema_venues(venues)
        typer.echo("Lokale zaktualizowane w lokalnej bazie danych.")

    @venues_app.command()
    def search(venue_name: Annotated[str, typer.Argument()] = "") -> None:
        found_venues = db_manager.find_venues_by_name(cinema_venue_input_parser(venue_name))
        db_venues_to_cli(found_venues, console)

    return app


def main() -> None:
    """Run the Typer application."""
    make_app()()


if __name__ == "__main__":
    main()

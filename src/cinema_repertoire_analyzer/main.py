import sys
from pathlib import Path
from typing import Annotated

import anyio
import httpx
import typer
from rich.console import Console

from cinema_repertoire_analyzer.cinema_api.models import (
    CinemaChainId,
    CinemaVenue,
    RepertoireCliTableMetadata,
)
from cinema_repertoire_analyzer.cinema_api.registry import (
    RegisteredCinemaChain,
    get_registered_chain,
)
from cinema_repertoire_analyzer.cli_utils import (
    cinema_venue_input_parser,
    date_input_parser,
    db_venues_to_cli,
    repertoire_to_cli,
)
from cinema_repertoire_analyzer.configuration import (
    ensure_settings_for_argv,
    load_settings,
    load_settings_if_available,
    run_interactive_configuration,
    should_defer_bootstrap_to_command,
    should_skip_bootstrap_for_argv,
)
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.exceptions import (
    AmbiguousVenueMatchError,
    AppError,
    DefaultVenueNotConfiguredError,
    VenueNotFoundError,
)
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails
from cinema_repertoire_analyzer.ratings_api.tmdb import get_movie_ratings_and_summaries
from cinema_repertoire_analyzer.settings import Settings


def _resolve_single_venue(found_venues: list[CinemaVenue]) -> CinemaVenue:
    """Resolve a venue search result to exactly one venue."""
    if not found_venues:
        raise VenueNotFoundError("Nie znaleziono żadnego lokalu o podanej nazwie.")
    if len(found_venues) > 1:
        raise AmbiguousVenueMatchError(len(found_venues))
    return found_venues[0]


def _resolve_chain(chain: str | None, settings: Settings) -> RegisteredCinemaChain:
    """Resolve CLI chain input to a registered chain definition."""
    chain_id = (
        settings.user_preferences.default_chain
        if chain is None
        else CinemaChainId.from_value(chain)
    )
    return get_registered_chain(chain_id)


def _resolve_venue_name(
    venue_name: str | None, chain: RegisteredCinemaChain, settings: Settings
) -> str:
    """Resolve the requested venue name, falling back to chain defaults."""
    if venue_name:
        return venue_name
    default_venue = settings.get_default_venue(chain.chain_id)
    if not default_venue:
        raise DefaultVenueNotConfiguredError(chain.display_name)
    return default_venue


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


def make_app(settings: Settings | None = None) -> typer.Typer:  # noqa: C901
    """Create the Typer application."""
    venues_app = typer.Typer()
    app = typer.Typer()
    app.add_typer(venues_app, name="venues")
    console = Console()
    runtime_settings = settings
    db_manager: DatabaseManager | None = None
    setattr(app, "_db_manager", None)

    def get_runtime_settings() -> Settings:
        nonlocal runtime_settings
        if runtime_settings is None:
            runtime_settings = load_settings()
        return runtime_settings

    def get_existing_settings() -> Settings | None:
        nonlocal runtime_settings
        if runtime_settings is not None:
            return runtime_settings
        runtime_settings = load_settings_if_available()
        return runtime_settings

    def reset_db_manager() -> None:
        nonlocal db_manager
        if db_manager is not None:
            db_manager.close()
            db_manager = None
        setattr(app, "_db_manager", None)

    def get_db_manager() -> DatabaseManager:
        nonlocal db_manager
        if db_manager is None:
            db_manager = _build_database_manager(get_runtime_settings().db_file)
            setattr(app, "_db_manager", db_manager)
        return db_manager

    @app.command()
    def configure() -> None:
        nonlocal runtime_settings
        try:
            runtime_settings = run_interactive_configuration(get_existing_settings())
            reset_db_manager()
        except AppError as error:
            _handle_cli_error(error)

    @app.command()
    def repertoire(
        chain: Annotated[str | None, typer.Option(help="Id sieci kin, np. cinema-city")] = None,
        venue_name: Annotated[str | None, typer.Argument()] = None,
        date: Annotated[str | None, typer.Argument()] = None,
    ) -> None:
        try:
            settings_instance = get_runtime_settings()
            registered_chain = _resolve_chain(chain, settings_instance)
            resolved_venue_name = _resolve_venue_name(
                venue_name, registered_chain, settings_instance
            )
            venue_name_parsed = cinema_venue_input_parser(resolved_venue_name)
            found_venues = get_db_manager().find_venues_by_name(
                registered_chain.chain_id, venue_name_parsed
            )
            venue = _resolve_single_venue(found_venues)
            date_parsed = date_input_parser(date or settings_instance.user_preferences.default_day)

            cinema_instance = registered_chain.client_factory(settings_instance)
            fetched_repertoire = anyio.run(cinema_instance.fetch_repertoire, date_parsed, venue)
            movie_titles = [repertoire.title for repertoire in fetched_repertoire]
            ratings = _load_tmdb_ratings(
                movie_titles, settings_instance.user_preferences.tmdb_access_token, console
            )

            table_metadata = RepertoireCliTableMetadata(
                chain_display_name=registered_chain.display_name,
                repertoire_date=date_parsed,
                cinema_venue_name=str(venue.venue_name),
            )
            repertoire_to_cli(fetched_repertoire, table_metadata, ratings, console)
        except AppError as error:
            _handle_cli_error(error)

    @venues_app.command()
    def list(
        chain: Annotated[str | None, typer.Option(help="Id sieci kin, np. cinema-city")] = None,
    ) -> None:
        try:
            settings_instance = get_runtime_settings()
            registered_chain = _resolve_chain(chain, settings_instance)
            venues = get_db_manager().get_all_venues(registered_chain.chain_id)
            db_venues_to_cli(venues, registered_chain.display_name, console)
        except AppError as error:
            _handle_cli_error(error)

    @venues_app.command()
    def update(
        chain: Annotated[str | None, typer.Option(help="Id sieci kin, np. cinema-city")] = None,
    ) -> None:
        try:
            settings_instance = get_runtime_settings()
            registered_chain = _resolve_chain(chain, settings_instance)
            typer.echo(f"Aktualizowanie lokali dla sieci: {registered_chain.display_name}...")
            cinema_instance = registered_chain.client_factory(settings_instance)
            venues = anyio.run(cinema_instance.fetch_venues)
            get_db_manager().replace_venues(registered_chain.chain_id, venues)
            typer.echo("Lokale zaktualizowane w lokalnej bazie danych.")
        except AppError as error:
            _handle_cli_error(error)

    @venues_app.command()
    def search(
        chain: Annotated[str | None, typer.Option(help="Id sieci kin, np. cinema-city")] = None,
        venue_name: Annotated[str, typer.Argument()] = "",
    ) -> None:
        try:
            settings_instance = get_runtime_settings()
            registered_chain = _resolve_chain(chain, settings_instance)
            found_venues = get_db_manager().find_venues_by_name(
                registered_chain.chain_id, cinema_venue_input_parser(venue_name)
            )
            db_venues_to_cli(found_venues, registered_chain.display_name, console)
        except AppError as error:
            _handle_cli_error(error)

    return app


def main() -> None:
    """Run the Typer application."""
    argv = sys.argv[1:]
    settings: Settings | None = None
    try:
        if should_skip_bootstrap_for_argv(argv) or should_defer_bootstrap_to_command(argv):
            settings = load_settings_if_available()
        else:
            settings = ensure_settings_for_argv(argv)
    except AppError as error:
        _handle_cli_error(error)

    make_app(settings)()


if __name__ == "__main__":
    main()

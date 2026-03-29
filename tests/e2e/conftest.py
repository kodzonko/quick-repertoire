import gc
import tempfile
import time
from collections.abc import Iterator
from pathlib import Path

import pytest
from typer import Typer
from typer.testing import CliRunner

import cinema_repertoire_analyzer.configuration as configuration_module
from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.main import make_app
from cinema_repertoire_analyzer.settings import (
    CinemaChainSettings,
    CinemaChainsSettings,
    DefaultVenues,
    Settings,
    UserPreferences,
)


def _cleanup_temp_dir(temp_dir_context: tempfile.TemporaryDirectory[str]) -> None:
    """Retry temp directory cleanup to handle delayed SQLite file releases on Windows."""
    last_error: PermissionError | None = None
    for _ in range(5):
        gc.collect()
        try:
            temp_dir_context.cleanup()
            return
        except PermissionError as error:
            last_error = error
            time.sleep(0.1)
    if last_error is not None:
        raise last_error


@pytest.fixture
def settings() -> Iterator[Settings]:
    temp_dir_context = tempfile.TemporaryDirectory(prefix="cinema-repertoire-analyzer-e2e-")
    temp_dir = Path(temp_dir_context.name)
    db_file = temp_dir / "test_db.sqlite"
    original_project_root = configuration_module.PROJECT_ROOT
    configuration_module.load_settings.cache_clear()
    settings_instance = Settings(
        db_file=db_file,
        loguru_level="TRACE",
        user_preferences=UserPreferences(
            default_chain=CinemaChainId.CINEMA_CITY,
            default_day="today",
            tmdb_access_token="1234",
            default_venues=DefaultVenues(cinema_city="Wroclaw - Wroclavia"),
        ),
        cinema_chains=CinemaChainsSettings(
            cinema_city=CinemaChainSettings(
                repertoire_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
                "in-cinema={cinema_venue_id}&at={repertoire_date}",
                venues_list_url="https://www.cinema-city.pl/#/buy-tickets-by-cinema",
            )
        ),
    )
    configuration_module.PROJECT_ROOT = temp_dir
    configuration_module._write_settings(settings_instance)
    db_manager = DatabaseManager(db_file_path=settings_instance.db_file)
    db_manager.replace_venues(
        "cinema-city",
        [
            CinemaVenue(
                chain_id="cinema-city", venue_name="Warszawa - Galeria Mokotow", venue_id="1"
            ),
            CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="2"),
            CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="3"),
        ],
    )
    db_manager.close()
    yield settings_instance
    configuration_module.PROJECT_ROOT = original_project_root
    configuration_module.load_settings.cache_clear()
    _cleanup_temp_dir(temp_dir_context)


@pytest.fixture
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture
def typer_app(settings: Settings) -> Iterator[Typer]:
    app = make_app()
    yield app
    manager = getattr(app, "_db_manager", None)
    if isinstance(manager, DatabaseManager):
        manager.close()


@pytest.fixture
def db_manager(settings: Settings) -> Iterator[DatabaseManager]:
    manager = DatabaseManager(db_file_path=settings.db_file)
    yield manager
    manager.close()

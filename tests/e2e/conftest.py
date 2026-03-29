import gc
import tempfile
import time
from collections.abc import Iterator
from pathlib import Path

import pytest
from typer import Typer
from typer.testing import CliRunner

from cinema_repertoire_analyzer.cinema_api.models import CinemaVenue
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.main import make_app
from cinema_repertoire_analyzer.settings import Settings, get_settings


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
def settings(monkeypatch: pytest.MonkeyPatch) -> Iterator[Settings]:
    temp_dir_context = tempfile.TemporaryDirectory(prefix="cinema-repertoire-analyzer-e2e-")
    temp_dir = Path(temp_dir_context.name)
    db_file = temp_dir / "test_db.sqlite"
    monkeypatch.delenv("ENV_PATH", raising=False)
    monkeypatch.setenv("LOGURU_LEVEL", "TRACE")
    monkeypatch.setenv("DB_FILE", str(db_file))
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_DAY", "today")
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_VENUES__CINEMA_CITY", "Wroclaw - Wroclavia")
    monkeypatch.setenv("USER_PREFERENCES__TMDB_ACCESS_TOKEN", "1234")
    monkeypatch.setenv(
        "CINEMA_CHAINS__CINEMA_CITY__REPERTOIRE_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}",
    )
    monkeypatch.setenv(
        "CINEMA_CHAINS__CINEMA_CITY__VENUES_LIST_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )
    get_settings.cache_clear()
    settings_instance = get_settings()
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
    get_settings.cache_clear()
    _cleanup_temp_dir(temp_dir_context)


@pytest.fixture
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture
def typer_app(settings: Settings) -> Iterator[Typer]:
    app = make_app(settings)
    yield app
    manager = getattr(app, "_db_manager", None)
    if isinstance(manager, DatabaseManager):
        manager.close()


@pytest.fixture
def db_manager(settings: Settings) -> Iterator[DatabaseManager]:
    manager = DatabaseManager(db_file_path=settings.db_file)
    yield manager
    manager.close()

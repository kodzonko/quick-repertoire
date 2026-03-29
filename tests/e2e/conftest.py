import shutil
from pathlib import Path
from uuid import uuid4

import pytest
from typer import Typer
from typer.testing import CliRunner

from conftest import RESOURCE_DIR
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.main import make_app
from cinema_repertoire_analyzer.settings import Settings, get_settings


@pytest.fixture
def settings(monkeypatch: pytest.MonkeyPatch) -> Settings:
    temp_dir = RESOURCE_DIR.parent.parent / ".codex-tmp" / "e2e-tests" / str(uuid4())
    temp_dir.mkdir(parents=True, exist_ok=True)
    db_file = temp_dir / "test_db.sqlite"
    shutil.copy(RESOURCE_DIR / "test_db.sqlite", db_file)
    monkeypatch.delenv("ENV_PATH", raising=False)
    monkeypatch.setenv("LOGURU_LEVEL", "TRACE")
    monkeypatch.setenv("DB_FILE", str(db_file))
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_CINEMA_VENUE", "Wrocław - Wroclavia")
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_DAY", "today")
    monkeypatch.setenv("USER_PREFERENCES__TMDB_ACCESS_TOKEN", "1234")
    monkeypatch.setenv(
        "CINEMA_CITY_SETTINGS__REPERTOIRE_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}",
    )
    monkeypatch.setenv(
        "CINEMA_CITY_SETTINGS__VENUES_LIST_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )
    get_settings.cache_clear()
    yield get_settings()
    get_settings.cache_clear()
    shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.fixture
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture
def typer_app(settings: Settings) -> Typer:
    return make_app(settings)


@pytest.fixture
def db_manager(settings: Settings) -> DatabaseManager:
    return DatabaseManager(db_file_path=settings.DB_FILE)

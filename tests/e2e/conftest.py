import os

import pytest
from typer import Typer
from typer.testing import CliRunner

from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.main import make_app
from cinema_repertoire_analyzer.settings import Settings, get_settings


@pytest.fixture(scope="module")
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture(scope="module")
def settings() -> Settings:
    if not (ENV_PATH := os.environ.get("ENV_PATH")) or not ENV_PATH.endswith("test.env"):
        raise ValueError("Env_PATH environment variable is not set or is not set to test.env file.")
    return get_settings()


@pytest.fixture(scope="module")
def typer_app(settings: Settings) -> Typer:
    return make_app(settings)


@pytest.fixture(scope="module")
def db_manager(settings: Settings) -> DatabaseManager:
    return DatabaseManager(db_file_path=settings.DB_FILE)

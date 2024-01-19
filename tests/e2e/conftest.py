from pathlib import Path

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
    test_config_file = Path(__file__).parents[1] / "resources" / "test_config.json"
    return get_settings(test_config_file)


@pytest.fixture(scope="module")
def typer_app(settings: Settings) -> Typer:
    return make_app(settings)


@pytest.fixture(scope="module")
def db_manager() -> DatabaseManager:
    test_db_path = Path(__file__).parent / "resources" / "test_db.sqlite"
    return DatabaseManager(db_file_path=test_db_path)

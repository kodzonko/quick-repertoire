import pytest
from typer import Typer
from typer.testing import CliRunner

from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.main import make_app
from cinema_repertoire_analyzer.settings import Settings


@pytest.fixture
def runner() -> CliRunner:
    return CliRunner()


@pytest.fixture
def typer_app(settings: Settings) -> Typer:
    return make_app(settings)


@pytest.fixture
def db_manager(settings: Settings) -> DatabaseManager:
    return DatabaseManager(db_file_path=settings.DB_FILE)

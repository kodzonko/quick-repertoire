import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.vcr
@pytest.mark.e2e
def test_update_venues_updates_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "update", "cinema-city"])
    assert result.exit_code == 0
    assert (
        "Aktualizowanie lokali dla kina: Cinema City...\n"
        "Lokale zaktualizowane w lokalnej bazie danych."
    ) in result.stdout


@pytest.mark.vcr
@pytest.mark.e2e
def test_update_venues_fails_on_incorrect_user_input(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "update", "wrong input"])
    assert result.exit_code == 2
    assert 'Invalid value: Kino "wrong input" nie jest wspierane.' in result.stdout

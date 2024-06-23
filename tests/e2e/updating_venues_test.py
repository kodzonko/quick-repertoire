import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_update_venues_updates_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "update"])
    assert result.exit_code == 0
    assert (
        "Aktualizowanie lokali dla kina: Cinema City...\n"
        "Lokale zaktualizowane w lokalnej bazie danych."
    ) in result.stdout

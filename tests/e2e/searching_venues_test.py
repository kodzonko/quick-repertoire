import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_search_venues_finds_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "search", "warszawa"])
    assert result.exit_code == 0
    assert "Znalezione lokale sieci Cinema City" in result.stdout
    assert "Warszawa - Galeria Mokotów" in result.stdout

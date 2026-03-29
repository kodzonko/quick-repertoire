import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_list_venues_lists_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "list"])
    assert result.exit_code == 0
    assert "Warszawa - Galeria Mokotow" in result.stdout

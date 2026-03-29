import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_search_venues_finds_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["venues", "search", "--chain", "cinema-city", "warszawa"])
    assert result.exit_code == 0
    assert "Warszawa - Galeria Mokotow" in result.stdout


@pytest.mark.e2e
def test_getting_repertoire_with_ambiguous_venue_returns_cli_error(
    typer_app: Typer, runner: CliRunner
) -> None:
    result = runner.invoke(typer_app, ["repertoire", "--chain", "cinema-city", "warszawa"])

    assert result.exit_code == 1
    assert "Podana nazwa lokalu jest niejednoznaczna." in result.stdout


@pytest.mark.e2e
def test_getting_repertoire_with_missing_venue_returns_cli_error(
    typer_app: Typer, runner: CliRunner
) -> None:
    result = runner.invoke(typer_app, ["repertoire", "--chain", "cinema-city", "to-nie-istnieje"])

    assert result.exit_code == 1
    assert "Nie znaleziono żadnego lokalu o podanej nazwie." in result.stdout

import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_update_venues_updates_venues_correctly(
    typer_app: Typer, runner: CliRunner
) -> None:
    result = runner.invoke(typer_app, ["update-venues", "cinema-city"])
    assert result.exit_code == 0
    assert "Updating venues for Cinema City..." in result.stdout
    assert "Venues updated in the local database." in result.stdout


@pytest.mark.e2e
def test_update_venues_fails_on_incorrect_user_input(
    typer_app: Typer, runner: CliRunner
) -> None:
    result = runner.invoke(typer_app, ["update-venues", "wrong input"])
    assert result.exit_code == 2
    assert 'Invalid value: "wrong input" is not supported.' in result.stdout

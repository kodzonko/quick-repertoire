import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_list_venues_lists_venues_correctly(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["list-venues", "cinema-city"])
    assert (
        "               Kina sieci Cinema City               \n"
        "┌──────────┬───────────────────────────────────────┐\n"
        "│ venue_id │ venue_name                            │\n"
        "├──────────┼───────────────────────────────────────┤\n" in result.stdout
    )
    assert result.exit_code == 0


@pytest.mark.e2e
def test_list_venues_fails_on_incorrect_user_input(typer_app: Typer, runner: CliRunner) -> None:
    result = runner.invoke(typer_app, ["list-venues", "wrong input"])
    assert result.exit_code == 2
    assert 'Invalid value: "wrong input" is not supported.' in result.stdout

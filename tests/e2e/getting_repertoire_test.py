import pytest
from typer import Typer
from typer.testing import CliRunner


@pytest.mark.e2e
def test_get_repertoire_with_default_values_returns_repertoire_correctly(
    typer_app: Typer, runner: CliRunner
) -> None:
    result = runner.invoke(typer_app, ["repertoire"])
    assert result.exit_code == 0
    assert "Repertuar dla Cinema City (Wrocław - Wroclavia) na dzień:" in result.stdout

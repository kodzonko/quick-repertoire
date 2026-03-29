import pytest
from typer import Typer
from typer.testing import CliRunner

import cinema_repertoire_analyzer.main as tested_module
from cinema_repertoire_analyzer.cinema_api.models import MoviePlayDetails, Repertoire


@pytest.mark.e2e
def test_get_repertoire_with_default_values_returns_repertoire_correctly(
    typer_app: Typer, runner: CliRunner, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(tested_module, "verify_api_key", lambda _: False)

    async def fake_fetch_repertoire(self, date, venue_data):
        return [
            Repertoire(
                title="Test Movie",
                genres="Thriller",
                play_length="120 min",
                original_language="EN",
                play_details=[
                    MoviePlayDetails(
                        format="2D",
                        play_language="NAP: PL",
                        play_times=["10:00", "12:30"],
                    )
                ],
            )
        ]

    monkeypatch.setattr(
        tested_module.CinemaCity,
        "fetch_repertoire",
        fake_fetch_repertoire,
    )

    result = runner.invoke(typer_app, ["repertoire"])

    assert result.exit_code == 0
    assert "Repertuar dla Cinema City" in result.stdout
    assert "Wroclavia" in result.stdout
    assert "Test Movie" in result.stdout

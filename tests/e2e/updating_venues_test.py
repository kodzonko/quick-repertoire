import pytest
from typer import Typer
from typer.testing import CliRunner

import cinema_repertoire_analyzer.main as tested_module
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.database.models import CinemaVenues


@pytest.mark.e2e
def test_update_venues_updates_venues_correctly(
    typer_app: Typer,
    runner: CliRunner,
    db_manager: DatabaseManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def fake_fetch_cinema_venues_list(self):
        return [CinemaVenues(venue_name="Test Venue", venue_id="9999")]

    monkeypatch.setattr(
        tested_module.CinemaCity,
        "fetch_cinema_venues_list",
        fake_fetch_cinema_venues_list,
    )

    result = runner.invoke(typer_app, ["venues", "update"])

    assert result.exit_code == 0
    assert (
        "Aktualizowanie lokali dla kina: Cinema City...\n"
        "Lokale zaktualizowane w lokalnej bazie danych."
    ) in result.stdout
    assert [(venue.venue_name, venue.venue_id) for venue in db_manager.get_all_venues()] == [
        ("Test Venue", "9999")
    ]

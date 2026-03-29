import pytest
from typer import Typer
from typer.testing import CliRunner

import cinema_repertoire_analyzer.cinema_api.registry as registry_module
from cinema_repertoire_analyzer.cinema_api.models import CinemaVenue
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager


@pytest.mark.e2e
def test_update_venues_updates_venues_correctly(
    typer_app: Typer,
    runner: CliRunner,
    db_manager: DatabaseManager,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def fake_fetch_venues(self):
        return [CinemaVenue(chain_id="cinema-city", venue_name="Test Venue", venue_id="9999")]

    monkeypatch.setattr(registry_module.CinemaCity, "fetch_venues", fake_fetch_venues)

    result = runner.invoke(typer_app, ["venues", "update"])

    assert result.exit_code == 0
    assert "Aktualizowanie lokali dla sieci: Cinema City..." in result.stdout
    assert "Lokale zaktualizowane w lokalnej bazie danych." in result.stdout
    assert [
        (venue.venue_name, venue.venue_id) for venue in db_manager.get_all_venues("cinema-city")
    ] == [("Test Venue", "9999")]

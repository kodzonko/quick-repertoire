from pathlib import Path

import pytest

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId
from cinema_repertoire_analyzer.settings import Settings, UserPreferences


@pytest.mark.unit
def test_settings_default_builds_expected_defaults(tmp_path: Path) -> None:
    settings = Settings.default(project_root=tmp_path)

    assert settings.db_file == tmp_path / "db.sqlite"
    assert settings.loguru_level == "INFO"
    assert settings.user_preferences.default_chain == CinemaChainId.CINEMA_CITY
    assert settings.user_preferences.default_day == "today"
    assert settings.user_preferences.default_venues.cinema_city == "Wroclaw - Wroclavia"
    assert settings.cinema_chains.cinema_city.repertoire_url.endswith(
        "in-cinema={cinema_venue_id}&at={repertoire_date}"
    )
    assert settings.cinema_chains.cinema_city.venues_list_url.endswith("buy-tickets-by-cinema")


@pytest.mark.unit
def test_user_preferences_blank_tmdb_token_is_normalized_to_none() -> None:
    preferences = UserPreferences(tmdb_access_token="   ")

    assert preferences.tmdb_access_token is None


@pytest.mark.unit
def test_settings_returns_default_venue_for_selected_chain(settings: Settings) -> None:
    assert settings.get_default_venue(CinemaChainId.CINEMA_CITY) == "Wroclaw - Wroclavia"

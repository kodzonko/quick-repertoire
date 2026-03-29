import os
from pathlib import Path
from unittest.mock import patch

import pytest
from mockito import mock, when

from cinema_repertoire_analyzer.settings import Settings, get_settings

RESOURCE_DIR = Path(__file__).parents[1] / "resources"


@pytest.fixture
def ENV_PATH() -> None:  # type: ignore[misc] # noqa: N802
    original_value = os.environ.get("ENV_PATH")
    os.environ["ENV_PATH"] = "/foo/bar/path/setting_file.env"
    yield
    if original_value is not None:
        os.environ["ENV_PATH"] = original_value
    else:
        os.unsetenv("ENV_PATH")


@pytest.fixture
def settings() -> Settings:
    return mock(Settings)  # type: ignore[no-any-return]


@pytest.fixture
def clear_cache() -> None:
    get_settings.cache_clear()


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_from_ENV_PATH_file(  # noqa: N802
    settings_patched,
    ENV_PATH: None,  # noqa: N803
    clear_cache: None,
) -> None:
    settings_patched.return_value = "settings_instance_from_file_under_ENV_PATH"
    when(Path).exists().thenReturn(True)

    assert get_settings() == "settings_instance_from_file_under_ENV_PATH"


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_from_default_env_file(
    mock_settings, clear_cache: None
) -> None:
    when(os.environ).get("ENV_PATH").thenReturn(None)
    when(Path).exists().thenReturn(True)
    mock_settings.return_value = "settings_instance_from_default_env_file"

    assert get_settings() == "settings_instance_from_default_env_file"


@pytest.mark.unit
@patch("cinema_repertoire_analyzer.settings.Settings")
def test_get_settings_returns_correct_settings_env_vars(mock_settings, clear_cache: None) -> None:
    when(os.environ).get("ENV_PATH").thenReturn(None)
    when(Path).exists().thenReturn(False)
    mock_settings.return_value = "settings_instance_from_env_vars"

    assert get_settings() == "settings_instance_from_env_vars"


@pytest.mark.unit
def test_settings_accepts_uppercase_env_vars_for_lowercase_attributes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("LOGURU_LEVEL", "TRACE")
    monkeypatch.setenv("DB_FILE", "C:/tmp/test_db.sqlite")
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_DAY", "today")
    monkeypatch.setenv("USER_PREFERENCES__DEFAULT_VENUES__CINEMA_CITY", "Wroclaw - Wroclavia")
    monkeypatch.setenv("USER_PREFERENCES__TMDB_ACCESS_TOKEN", "1234")
    monkeypatch.setenv(
        "CINEMA_CHAINS__CINEMA_CITY__REPERTOIRE_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}",
    )
    monkeypatch.setenv(
        "CINEMA_CHAINS__CINEMA_CITY__VENUES_LIST_URL",
        "https://www.cinema-city.pl/#/buy-tickets-by-cinema",
    )

    settings = Settings(_env_nested_delimiter="__")

    assert settings.db_file == Path("C:/tmp/test_db.sqlite")
    assert settings.loguru_level == "TRACE"
    assert settings.user_preferences.default_day == "today"
    assert settings.user_preferences.tmdb_access_token == "1234"
    assert settings.user_preferences.default_venues.cinema_city == "Wroclaw - Wroclavia"
    assert (
        settings.cinema_chains.cinema_city.repertoire_url
        == "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}"
    )
    assert (
        settings.cinema_chains.cinema_city.venues_list_url
        == "https://www.cinema-city.pl/#/buy-tickets-by-cinema"
    )


@pytest.mark.unit
def test_settings_accepts_uppercase_env_file_keys_for_lowercase_attributes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("ENV_PATH", raising=False)
    monkeypatch.delenv("LOGURU_LEVEL", raising=False)
    monkeypatch.delenv("DB_FILE", raising=False)
    monkeypatch.delenv("USER_PREFERENCES__DEFAULT_DAY", raising=False)
    monkeypatch.delenv("USER_PREFERENCES__DEFAULT_VENUES__CINEMA_CITY", raising=False)
    monkeypatch.delenv("USER_PREFERENCES__TMDB_ACCESS_TOKEN", raising=False)
    monkeypatch.delenv("CINEMA_CHAINS__CINEMA_CITY__REPERTOIRE_URL", raising=False)
    monkeypatch.delenv("CINEMA_CHAINS__CINEMA_CITY__VENUES_LIST_URL", raising=False)

    settings = Settings(
        _env_file=RESOURCE_DIR / "test.env.template",
        _env_file_encoding="utf-8",
        _env_nested_delimiter="__",
    )

    assert settings.db_file == Path("C:/path/to/test_db.sqlite")
    assert settings.loguru_level == "INFO"
    assert settings.user_preferences.default_day in {"dziś", "dziĹ›"}
    assert settings.user_preferences.tmdb_access_token == "1234"
    assert settings.user_preferences.default_venues.cinema_city in {
        "Wrocław - Wroclavia",
        "WrocĹ‚aw - Wroclavia",
    }
    assert (
        settings.cinema_chains.cinema_city.repertoire_url
        == "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
        "in-cinema={cinema_venue_id}&at={repertoire_date}"
    )
    assert (
        settings.cinema_chains.cinema_city.venues_list_url
        == "https://www.cinema-city.pl/#/buy-tickets-by-cinema"
    )

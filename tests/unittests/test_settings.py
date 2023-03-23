from pathlib import Path
from typing import Any

import pytest
import toml
from mockito import when

from enums import CinemaChain
from exceptions import SettingsLoadError
from settings import load_config_for_cinema, load_settings


@pytest.fixture
def loaded_settings_correct() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Multikino",
            "default_cinema_venue": "some venue",
            "default_day": "today",
        },
        "db": {"db_file_path": "/some/path.ext"},
        "cinemas": {
            "Cinema City": {
                "repertoire_url": "https://www.example.com/cinema_city/repertoire",
                "venues_list_url": "https://www.example.com/cinema_city/venues",
            },
            "Helios": {
                "repertoire_url": "https://www.example.com/helios/repertoire",
                "venues_list_url": "https://www.example.com/helios/venues",
            },
            "Multikino": {
                "repertoire_url": "https://www.example.com/multikino/repertoire",
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        },
    }


@pytest.fixture
def loaded_settings_config_with_missing_values() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_day": "today",
        },
        "db": {},
    }


@pytest.fixture
def loaded_settings_incorrect_default_day_value() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Multikino",
            "default_cinema_venue": "some venue",
            "default_day": "yesterday",
        },
        "db": {"db_file_path": "/some/path.ext"},
        "cinemas": {
            "cinema_city": {
                "repertoire_url": "https://www.example.com/cinema_city/repertoire",
                "venues_list_url": "https://www.example.com/cinema_city/venues",
            },
            "helios": {
                "repertoire_url": "https://www.example.com/helios/repertoire",
                "venues_list_url": "https://www.example.com/helios/venues",
            },
            "multikino": {
                "repertoire_url": "https://www.example.com/multikino/repertoire",
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        },
    }


@pytest.fixture
def loaded_settings_incorrect_default_cinema_value() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Some Cinema",
            "default_cinema_venue": "some venue",
            "default_day": "today",
        },
        "db": {"db_file_path": "/some/path.ext"},
        "cinemas": {
            "cinema_city": {
                "repertoire_url": "https://www.example.com/cinema_city/repertoire",
                "venues_list_url": "https://www.example.com/cinema_city/venues",
            },
            "helios": {
                "repertoire_url": "https://www.example.com/helios/repertoire",
                "venues_list_url": "https://www.example.com/helios/venues",
            },
            "multikino": {
                "repertoire_url": "https://www.example.com/multikino/repertoire",
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        },
    }


@pytest.fixture
def loaded_settings_incorrect_db_path() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Multikino",
            "default_cinema_venue": "some venue",
            "default_day": "today",
        },
        "db": {"db_file_path": "/non/existing/path.ext"},
        "cinemas": {
            "cinema_city": {
                "repertoire_url": "https://www.example.com/cinema_city/repertoire",
                "venues_list_url": "https://www.example.com/cinema_city/venues",
            },
            "helios": {
                "repertoire_url": "https://www.example.com/helios/repertoire",
                "venues_list_url": "https://www.example.com/helios/venues",
            },
            "multikino": {
                "repertoire_url": "https://www.example.com/multikino/repertoire",
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        },
    }


@pytest.fixture
def loaded_settings_incomplete_cinema_config() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Multikino",
            "default_cinema_venue": "some venue",
            "default_day": "yesterday",
        },
        "db": {"db_file_path": "/some/path.ext"},
        "cinemas": {
            "cinema_city": {},
            "helios": {
                "repertoire_url": "https://www.example.com/helios/repertoire",
            },
            "multikino": {
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        },
    }


def test_load_settings_parses_correct_config(
    loaded_settings_correct: dict[str, Any]
) -> None:
    expected = {
        "DEFAULT_CINEMA": CinemaChain("Multikino"),
        "DEFAULT_CINEMA_VENUE": "some venue",
        "DEFAULT_DAY": "today",
        "DB_FILE_PATH": Path("/some/path.ext"),
    }

    when(toml).load("/dummy/path.toml").thenReturn(loaded_settings_correct)
    when(Path).exists().thenReturn(True)

    assert load_settings("/dummy/path.toml") == expected


def test_load_settings_raises_error_on_config_with_missing_values(
    loaded_settings_config_with_missing_values: dict[str, Any]
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(
        loaded_settings_config_with_missing_values
    )
    when(Path).exists().thenReturn(True)

    with pytest.raises(
        SettingsLoadError,
        match="Settings file doesn't contain value for: 'default_cinema_venue'.",
    ):
        load_settings("/dummy/path.toml")


def test_load_settings_raises_error_on_config_with_incorrect_default_day(
    loaded_settings_incorrect_default_day_value: dict[str, Any]
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(
        loaded_settings_incorrect_default_day_value
    )

    with pytest.raises(
        SettingsLoadError,
        match=(
            'DEFAULT_DAY value: "yesterday" in /dummy/path.toml is invalid. Must be '
            'either "today" or "tomorrow".'
        ),
    ):
        load_settings("/dummy/path.toml")


def test_load_settings_raises_error_on_config_with_incorrect_default_cinema(
    loaded_settings_incorrect_default_cinema_value: dict[str, Any]
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(
        loaded_settings_incorrect_default_cinema_value
    )

    with pytest.raises(
        SettingsLoadError,
        match=(
            'DEFAULT_CINEMA value: "Some Cinema" in /dummy/path.toml is invalid. Must'
            ' be one of the following: "Cinema City", "Helios", "Multikino".'
        ),
    ):
        load_settings("/dummy/path.toml")


def test_load_settings_raises_error_on_config_with_non_existing_db_file_path(
    loaded_settings_incorrect_db_path: dict[str, Any]
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(loaded_settings_incorrect_db_path)

    with pytest.raises(
        SettingsLoadError,
        match=(
            'DB_FILE_PATH value: ".*" in .*/dummy/path.toml is invalid. File doesn\'t'
            " exist."
        ),
    ):
        load_settings("/dummy/path.toml")


def test_load_settings_raises_error_on_non_existing_config_file() -> None:
    with pytest.raises(
        SettingsLoadError,
        match=(
            'Failed to load settings. Adjust path: "/dummy/path.toml" or check '
            "permissions."
        ),
    ):
        load_settings("/dummy/path.toml")


@pytest.mark.parametrize(
    "cinema_chain, expected",
    [
        (
            CinemaChain.CINEMA_CITY,
            {
                "repertoire_url": "https://www.example.com/cinema_city/repertoire",
                "venues_list_url": "https://www.example.com/cinema_city/venues",
            },
        ),
        (
            CinemaChain.MULTIKINO,
            {
                "repertoire_url": "https://www.example.com/multikino/repertoire",
                "venues_list_url": "https://www.example.com/multikino/venues",
            },
        ),
        (
            CinemaChain.HELIOS,
            {
                "repertoire_url": "https://www.example.com/helios/repertoire",
                "venues_list_url": "https://www.example.com/helios/venues",
            },
        ),
    ],
)
def test_load_config_for_cinema_loads_correct_config(
    loaded_settings_correct: dict[str, Any],
    cinema_chain: CinemaChain,
    expected: dict[str, Any],
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(loaded_settings_correct)

    assert load_config_for_cinema(cinema_chain, "/dummy/path.toml") == expected


def test_load_config_for_cinema_raises_error_on_non_existing_config_file() -> None:
    with pytest.raises(
        SettingsLoadError,
        match=(
            'Failed to load settings. Adjust path: "/dummy/path.toml" or check '
            "permissions."
        ),
    ):
        load_config_for_cinema(CinemaChain.CINEMA_CITY, "/dummy/path.toml")


@pytest.mark.parametrize(
    "cinema_chain, error_message",
    [
        (
            CinemaChain.CINEMA_CITY,
            "Settings file doesn't contain value for: 'Cinema City'.",
        ),
        (
            CinemaChain.MULTIKINO,
            "Settings file doesn't contain value for: 'Multikino'.",
        ),
        (CinemaChain.HELIOS, "Settings file doesn't contain value for: 'Helios'."),
    ],
)
def test_load_config_for_cinema_raises_error_on_incomplete_config(
    loaded_settings_incomplete_cinema_config: dict[str, Any],
    cinema_chain: CinemaChain,
    error_message: str,
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(
        loaded_settings_incomplete_cinema_config
    )

    with pytest.raises(
        SettingsLoadError,
        match=error_message,
    ):
        load_config_for_cinema(cinema_chain, "/dummy/path.toml")


def test_load_config_for_cinema_raises_error_on_config_with_missing_cinemas_table(
    loaded_settings_config_with_missing_values: dict[str, Any]
) -> None:
    when(toml).load("/dummy/path.toml").thenReturn(
        loaded_settings_config_with_missing_values
    )

    with pytest.raises(
        SettingsLoadError,
        match="Settings file doesn't contain value for: 'cinemas'.",
    ):
        load_config_for_cinema(CinemaChain.CINEMA_CITY, "/dummy/path.toml")

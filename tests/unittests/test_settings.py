from pathlib import Path
from typing import Any

import pytest
import toml
from mockito import when

from enums import CinemaChain
from exceptions import SettingsLoadError
from settings import load_settings

pytestmark = pytest.mark.usefixtures("unstub")


@pytest.fixture
def loaded_settings_correct() -> dict[str, Any]:
    return {
        "user_preferences": {
            "default_cinema": "Multikino",
            "default_cinema_venue": "some venue",
            "default_day": "today",
        },
        "db": {"db_file_path": "/some/path.ext"},
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
    loaded_settings_config_with_missing_values,
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
    loaded_settings_incorrect_default_day_value,
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
    loaded_settings_incorrect_default_cinema_value,
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
    loaded_settings_incorrect_db_path,
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
    when(toml).load("/dummy/path.toml").thenRaise(FileNotFoundError)
    with pytest.raises(
        SettingsLoadError,
        match=(
            "Failed to load settings. Adjust path: /dummy/path.toml or check"
            " permissions."
        ),
    ):
        load_settings("/dummy/path.toml")

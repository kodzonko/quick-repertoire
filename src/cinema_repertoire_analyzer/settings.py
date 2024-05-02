import json
from enum import StrEnum, auto
from functools import lru_cache
from pathlib import Path
from typing import TypeAlias, Any

from pydantic import AnyHttpUrl, PrivateAttr, computed_field
from pydantic_settings import BaseSettings

from cinema_repertoire_analyzer.enums import CinemaChain

PROJECT_ROOT = Path(__file__).parents[2]
_SETTINGS_FILE = PROJECT_ROOT / "config.json"


class _AllowedDefaultDays(StrEnum):
    TODAY = auto()
    TOMORROW = auto()


class UserPreferences(BaseSettings):
    default_cinema: CinemaChain
    default_cinema_venue: str
    default_day: _AllowedDefaultDays
    _db_file_path_relative: str = PrivateAttr()

    @computed_field  # type: ignore[misc]
    @property
    def db_file_path(self) -> Path:
        return PROJECT_ROOT / self._db_file_path_relative

    def __init__(self, _db_file_path_relative: str, **kwargs: Any) -> None:
        super().__init__(**kwargs)
        self._db_file_path_relative = _db_file_path_relative


class CinemaCitySettings(BaseSettings):
    repertoire_url: AnyHttpUrl
    venues_list_url: AnyHttpUrl


class HeliosSettings(BaseSettings):
    repertoire_url: AnyHttpUrl
    venues_list_url: AnyHttpUrl


class MultikinoSettings(BaseSettings):
    repertoire_url: AnyHttpUrl
    venues_list_url: AnyHttpUrl


class Settings(BaseSettings):
    """Settings for the application."""

    user_preferences: UserPreferences
    cinema_city_settings: CinemaCitySettings
    helios_settings: CinemaCitySettings
    multikino_settings: MultikinoSettings

    @classmethod
    def from_file(cls, file_path: str):
        with open(file_path, encoding="utf-8") as file:
            data = json.load(file)
        return cls.model_validate(data)


@lru_cache
def get_settings(file_path: Path = _SETTINGS_FILE) -> Settings:
    """Get the settings for the application."""
    return Settings.from_file(str(file_path))


CinemaSettings: TypeAlias = CinemaCitySettings | HeliosSettings | MultikinoSettings

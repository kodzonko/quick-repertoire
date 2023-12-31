import json
from enum import StrEnum, auto
from pathlib import Path

from pydantic import AnyHttpUrl
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
    db_file_path: Path


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

    class Config:
        extra = "ignore"


settings = Settings.from_file(str(_SETTINGS_FILE))

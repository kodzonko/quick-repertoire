import os
import sys
from functools import lru_cache
from pathlib import Path
from typing import Any, Literal

import typer
from loguru import logger
from pydantic import BaseModel, Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

PROJECT_ROOT = Path(__file__).parents[2]

AllowedDefaultDays = Literal["dziś", "dzis", "dzisiaj", "today", "jutro", "tomorrow"]
LOG_LVLS = Literal["TRACE", "WARNING", "DEBUG", "INFO", "ERROR", "CRITICAL"]


class DefaultVenues(BaseModel):
    """Per-chain default venue names."""

    cinema_city: str | None = None


class UserPreferences(BaseModel):
    """User preferences for the application."""

    default_day: AllowedDefaultDays
    tmdb_access_token: str | None = None
    default_venues: DefaultVenues = Field(default_factory=DefaultVenues)


class CinemaChainSettings(BaseModel):
    """Settings for a single cinema chain adapter."""

    repertoire_url: str
    venues_list_url: str


class CinemaChainsSettings(BaseModel):
    """Settings for all configured cinema chains."""

    cinema_city: CinemaChainSettings


class Settings(BaseSettings):
    """Settings for the application."""

    db_file: Path
    user_preferences: UserPreferences
    cinema_chains: CinemaChainsSettings
    loguru_level: LOG_LVLS = "INFO"

    @field_validator("loguru_level")
    def set_env_for_loguru(cls, loguru_level: LOG_LVLS) -> LOG_LVLS:  # noqa: N805
        """Set the loguru handler according to log level.

        This handles clearing loguru log handlers and adding the one with appropriate log level.
        """
        logger.remove()
        logger.add(sys.stdout, level=loguru_level)
        return loguru_level

    model_config = SettingsConfigDict(extra="allow")


def _build_settings(**kwargs: Any) -> Settings:
    """Instantiate settings with pydantic-specific loading kwargs."""
    return Settings(**kwargs)


@lru_cache
def get_settings() -> Settings:
    """Get the settings for the application."""
    ENV_PATH = None  # noqa: N806
    if os.environ.get("ENV_PATH") and Path(os.environ["ENV_PATH"]).exists():
        ENV_PATH = Path(os.environ["ENV_PATH"])  # noqa: N806
    elif Path(PROJECT_ROOT / "run.env").exists():
        ENV_PATH = PROJECT_ROOT / "run.env"  # noqa: N806
    else:
        typer.echo(f"Podany plik konfiguracyjny: {ENV_PATH=} nie istnieje.")
        # attempt loading variables from environment
        return _build_settings(_env_nested_delimiter="__")
    return _build_settings(
        _env_file=ENV_PATH, _env_file_encoding="utf-8", _env_nested_delimiter="__"
    )

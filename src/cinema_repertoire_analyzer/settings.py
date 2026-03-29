import sys
from pathlib import Path
from typing import Literal

from loguru import logger
from pydantic import BaseModel, Field, field_validator

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId

PROJECT_ROOT = Path(__file__).parents[2]
DEFAULT_CINEMA_CITY_REPERTOIRE_URL = (
    "https://www.cinema-city.pl/#/buy-tickets-by-cinema?"
    "in-cinema={cinema_venue_id}&at={repertoire_date}"
)
DEFAULT_CINEMA_CITY_VENUES_LIST_URL = "https://www.cinema-city.pl/#/buy-tickets-by-cinema"

AllowedDefaultDays = Literal["today", "tomorrow"]
LOG_LVLS = Literal["TRACE", "WARNING", "DEBUG", "INFO", "ERROR", "CRITICAL"]


def _chain_attr_name(chain_id: CinemaChainId | str) -> str:
    chain_value = chain_id.value if isinstance(chain_id, CinemaChainId) else chain_id
    return chain_value.replace("-", "_")


class DefaultVenues(BaseModel):
    """Per-chain default venue names."""

    cinema_city: str | None = "Wroclaw - Wroclavia"

    def get(self, chain_id: CinemaChainId) -> str | None:
        """Return a default venue name for a registered chain."""
        return getattr(self, _chain_attr_name(chain_id))


class UserPreferences(BaseModel):
    """User preferences for the application."""

    default_chain: CinemaChainId = CinemaChainId.CINEMA_CITY
    default_day: AllowedDefaultDays = "today"
    tmdb_access_token: str | None = None
    default_venues: DefaultVenues = Field(default_factory=DefaultVenues)

    @field_validator("tmdb_access_token", mode="before")
    @classmethod
    def blank_tmdb_access_token_to_none(cls, value: str | None) -> str | None:
        """Treat blank TMDB token values as disabled configuration."""
        if value is None:
            return None
        stripped_value = value.strip()
        return stripped_value or None


class CinemaChainSettings(BaseModel):
    """Settings for a single cinema chain adapter."""

    repertoire_url: str = DEFAULT_CINEMA_CITY_REPERTOIRE_URL
    venues_list_url: str = DEFAULT_CINEMA_CITY_VENUES_LIST_URL


class CinemaChainsSettings(BaseModel):
    """Settings for all configured cinema chains."""

    cinema_city: CinemaChainSettings = Field(default_factory=CinemaChainSettings)

    def get(self, chain_id: CinemaChainId) -> CinemaChainSettings:
        """Return settings for a registered chain."""
        return getattr(self, _chain_attr_name(chain_id))


class Settings(BaseModel):
    """Settings for the application."""

    db_file: Path = Field(default_factory=lambda: PROJECT_ROOT / "db.sqlite")
    user_preferences: UserPreferences = Field(default_factory=UserPreferences)
    cinema_chains: CinemaChainsSettings = Field(default_factory=CinemaChainsSettings)
    loguru_level: LOG_LVLS = "INFO"

    @field_validator("loguru_level")
    @classmethod
    def set_env_for_loguru(cls, loguru_level: LOG_LVLS) -> LOG_LVLS:
        """Set the loguru handler according to log level."""
        logger.remove()
        logger.add(sys.stdout, level=loguru_level)
        return loguru_level

    @classmethod
    def default(cls, *, project_root: Path | None = None) -> Settings:
        """Build a settings instance from built-in defaults."""
        resolved_project_root = project_root or PROJECT_ROOT
        return cls(db_file=resolved_project_root / "db.sqlite")

    def get_default_venue(self, chain_id: CinemaChainId) -> str | None:
        """Return the configured default venue for a selected chain."""
        return self.user_preferences.default_venues.get(chain_id)

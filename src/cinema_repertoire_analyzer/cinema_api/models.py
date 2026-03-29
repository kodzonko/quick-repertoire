from enum import StrEnum

from pydantic import BaseModel

from cinema_repertoire_analyzer.exceptions import UnsupportedCinemaChainError


class CinemaChainId(StrEnum):
    """Supported cinema chain identifiers exposed in the CLI."""

    CINEMA_CITY = "cinema-city"

    @classmethod
    def from_value(cls, value: str) -> CinemaChainId:
        """Parse a chain id from CLI input."""
        normalized_value = value.strip().lower()
        try:
            return cls(normalized_value)
        except ValueError as error:
            raise UnsupportedCinemaChainError(
                invalid_chain=value, supported_chains=", ".join(chain.value for chain in cls)
            ) from error


class CinemaVenue(BaseModel):
    """Cinema venue stored and exchanged across the application."""

    chain_id: str
    venue_id: str
    venue_name: str


class MoviePlayDetails(BaseModel):
    """Dictionary structure of movie play details as parsed from the cinema website."""

    format: str
    play_language: str
    play_times: list[str]


class Repertoire(BaseModel):
    """Dictionary structure of repertoire as parsed from the cinema website."""

    title: str
    genres: str
    play_length: str
    original_language: str
    play_details: list[MoviePlayDetails]


class RepertoireCliTableMetadata(BaseModel):
    """Metadata for the repertoire table representation in CLI."""

    chain_display_name: str
    repertoire_date: str
    cinema_venue_name: str

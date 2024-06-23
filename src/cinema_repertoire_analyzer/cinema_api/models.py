from pydantic import BaseModel


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

    repertoire_date: str
    cinema_venue_name: str

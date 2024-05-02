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
    play_length: int
    original_language: str | None
    play_details: list[MoviePlayDetails]

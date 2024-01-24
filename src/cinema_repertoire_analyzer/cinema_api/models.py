from typing import TypedDict


class MoviePlayDetails(TypedDict):
    """Dictionary structure of movie play details as parsed from the cinema website."""

    format: str
    play_language: str
    play_times: list[str]


class Repertoire(TypedDict):
    """Dictionary structure of repertoire as parsed from the cinema website."""

    title: str
    genres: str
    play_length: int
    original_language: str
    play_details: list[MoviePlayDetails]

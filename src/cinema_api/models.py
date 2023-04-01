from typing import TypedDict


class Repertoire(TypedDict):
    """Dictionary structure of repertoire as parsed from the cinema website."""


class CinemaVenues(TypedDict):
    """Dictionary structure of cinema venues as parsed from the cinema website."""

    name: str
    id: int


class CinemaConfig(TypedDict):
    """Dictionary structure of cinema configuration as parsed from the config file."""


class MoviePlayDetails(TypedDict):
    """Dictionary structure of movie play details as parsed from the cinema website."""

    format: str
    play_language: str
    play_times: list[str]

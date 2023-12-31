from typing import TypedDict

from cinema_repertoire_analyzer.enums import CinemaChain


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


class CinemaVenue(TypedDict):
    """Dictionary structure of cinema venues as parsed from the cinema website."""

    name: str
    id: int


class CinemaConfig(TypedDict):
    """Dictionary structure of cinema configuration as parsed from the config file."""

    cinema_chain: CinemaChain
    repertoire_url: str
    cinema_venues_url: str

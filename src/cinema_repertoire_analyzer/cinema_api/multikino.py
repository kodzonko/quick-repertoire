from datetime import datetime

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.models import CinemaVenue, Repertoire
from cinema_repertoire_analyzer.enums import CinemaChain


class Multikino(Cinema):
    """Class handling interactions with www.multikino.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.MULTIKINO
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        pass

    def fetch_cinema_venues_list(self) -> list[CinemaVenue]:
        pass

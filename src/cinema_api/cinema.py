"""Module containing logic interacting with cinema websites.

Due to lack of official APIs for the supported cinemas, data is downloaded using web
scraping. This module is the de-facto simplified API for the supported cinemas.

Be mindful of the fact that the websites may change their structure at any time,
urls have been abstracted away to the config file, but the logic may need to be affected
as well.

Don't overuse these functions, as too many requests may result in a ban from the
website.
"""
from datetime import datetime
from typing import Protocol

from requests_html import HTMLSession

import utils
from cinema_api.models import CinemaConfig, CinemaVenues, Repertoire
from enums import CinemaChain
from settings import load_config_for_cinema


class Cinema(Protocol):
    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        """Download repertoire for a specified date from the cinema website."""

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        """Download list of cinema venues from the cinema website."""


class CinemaCity:
    """Class handling interactions with www.cinema-city.pl website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.CINEMA_CITY
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url

    def fetch_repertoire(self, date: datetime, venue_id: int) -> list[Repertoire]:
        """Download repertoire for a specified date and venue from the cinema website.
        """
        repertoire_date = date.strftime("%Y-%m-%d")  # ????
        session = HTMLSession()
        url = utils.fill_string_template(
            self.repertoire_url, venue_id=venue_id, date=repertoire_date
        )
        response = session.get(url)
        response.html.render()  # render JS elements
        films = response.html.find(selector="h3.qb-movie-name")

        output: list[Repertoire] = []
        for f in films:
            output.append(
                {
                    "title": f.text,
                    "time": "???",
                    "language": "???",
                    "format": "???",
                }
            )

        return [film.text for film in films]

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        """Download list of cinema venues from the cinema website."""
        session = HTMLSession()
        response = session.get(self.cinema_venues_url, verify=False)
        response.html.render()  # render JS elements
        cinemas = response.html.find(selector="option[value][data-tokens]")
        venues = [cinema.element.get("data-tokens") for cinema in cinemas]
        ids = [int(cinema.element.get("value")) for cinema in cinemas]

        output: list[CinemaVenues] = []
        for venue, id_ in zip(venues, ids):
            output.append({"name": venue, "id": id_})

        return output


class Helios:
    """Class handling interactions with Helios website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.HELIOS
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url
        raise NotImplementedError

    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        raise NotImplementedError

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        raise NotImplementedError


class Multikino:
    """Class handling interactions with Multikino website."""

    def __init__(self, repertoire_url: str, cinema_venues_url: str) -> None:
        self.cinema_chain = CinemaChain.MULTIKINO
        self.repertoire_url = repertoire_url
        self.cinema_venues_url = cinema_venues_url
        raise NotImplementedError

    def fetch_repertoire(self, date: datetime, cinema_venue: str) -> list[Repertoire]:
        raise NotImplementedError

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        raise NotImplementedError


def cinema_factory(cinema_chain: CinemaChain) -> Cinema:
    config: CinemaConfig = load_config_for_cinema(cinema_chain)
    enum_class_mapping = {
        CinemaChain.CINEMA_CITY: CinemaCity,
        CinemaChain.HELIOS: Helios,
        CinemaChain.MULTIKINO: Multikino,
    }

    return enum_class_mapping[cinema_chain](**config)

"""Module containing logic interacting with cinema websites.

Due to lack of official APIs for the supported cinemas, data is downloaded using web
scraping. This module is the de-facto simplified API for the supported cinemas.

Be mindful of the fact that the websites may change their structure at any time,
urls have been abstracted away to the config file, but the logic may need to be affected
as well.

Don't overuse these functions, as too many requests may result in a ban from the
website.
"""
import re
import sys
from datetime import datetime
from typing import Protocol

from bs4 import BeautifulSoup
from requests_html import Element, HTMLResponse, HTMLSession

import utils
from cinema_api.models import CinemaConfig, CinemaVenues, MoviePlayDetails, Repertoire
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
        repertoire_date = date.strftime("%Y-%m-%d")
        session = HTMLSession()
        url = utils.fill_string_template(
            self.repertoire_url,
            cinema_venue_id=venue_id,
            repertoire_date=repertoire_date,
        )
        response: HTMLResponse = session.get(url)
        response.html.render()  # render JS elements
        session.close()  # otherwise Chromium process will leak
        soup = BeautifulSoup(response.html.html, "lxml")
        output = []
        movies_details: list[Element] = soup.find_all("div", class_="qb-movie-details")
        for movie in movies_details:
            if movie.find("h4", attrs={"text": "KUP BILET W PRZEDSPRZEDAŻY"}) is None:
                continue
            output.append(
                {
                    "title": self._parse_title(movie),
                    "genres": self._parse_genres(movie),
                    "play_length": self._parse_play_length(movie),
                    "original_language": self._parse_original_language(movie),
                    "play_details": self._parse_play_details(movie),
                }
            )

        return output

    def fetch_cinema_venues_list(self) -> list[CinemaVenues]:
        """Download list of cinema venues from the cinema website."""
        session = HTMLSession()
        response = session.get(self.cinema_venues_url, verify=False)
        response.html.render()  # render JS elements
        cinemas = response.html.find("option[value][data-tokens]")
        venues = [cinema.element.get("data-tokens") for cinema in cinemas]
        ids = [int(cinema.element.get("value")) for cinema in cinemas]

        output: list[CinemaVenues] = []
        for venue, id_ in zip(venues, ids):
            output.append({"name": venue, "id": id_})

        return output

    def _parse_title(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract title."""
        return html.find("h3", "qb-movie-name").text.strip()

    def _parse_genres(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract genres."""
        raw_str = html.find("div", class_="qb-movie-info").find("span").text
        return raw_str.replace("|", "").strip()

    def _parse_original_language(self, html: Element) -> str | None:
        """Parse HTML element of a single movie to extract original language."""
        element = html.find("span", attrs={"aria-label": re.compile("original-lang")})
        return element.text.strip() if element else None

    def _parse_play_length(self, html: Element) -> int:
        """Parse HTML element of a single movie to extract play length."""
        time_raw = html.find("div", class_="qb-movie-info").find_all("span")[1].text
        time_raw = time_raw.strip()
        return int(re.sub(r"\D", "", time_raw))

    def _parse_play_format(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play format."""
        formats_section = html.find("ul", class_="qb-screening-attributes")
        try:
            formats = formats_section.find_all(
                "span", attrs={"aria-label": re.compile("Screening type")}
            )
        except AttributeError:
            import pdb

            pdb.set_trace()
            sys.exit()

        return " ".join([f.text.strip() for f in formats])

    def _parse_play_times(self, html: Element) -> list[str]:
        """Parse HTML element of a single movie to extract play times."""
        times = html.find_all("a", class_="btn btn-primary btn-lg")
        return [t.text.strip() for t in times]

    def _parse_play_language(self, html: Element) -> str:
        """Parse HTML element of a single movie to extract play language."""
        sub_dub_or_original_prefix = html.find(
            "span", attrs={"aria-label": re.compile("subAbbr|dubAbbr|noSubs")}
        )
        language = html.find(
            "span", attrs={"aria-label": re.compile("subbed-lang|dubbed-lang")}
        )

        return (
            f"{sub_dub_or_original_prefix.text.strip()}{': ' if language else ''}{language.text.strip() if language else ''}"
        )

    def _parse_play_details(self, html: Element) -> list[MoviePlayDetails]:
        """Parse HTML element of a single movie to extract play formats, languages and respective play times.
        """
        output = []
        play_details = html.find_all("div", class_="qb-movie-info-column")
        for html in play_details:
            output.append(
                {
                    "format": self._parse_play_format(html),
                    "play_times": self._parse_play_times(html),
                    "play_language": self._parse_play_language(html),
                }
            )
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

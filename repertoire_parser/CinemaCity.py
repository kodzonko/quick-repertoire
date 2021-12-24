import json
import re
from datetime import date
from json import JSONDecodeError
from pathlib import Path, PurePath
from typing import List

import requests
from loguru import logger
from requests_html import HTMLSession

from settings.Settings import Settings


class CinemaCity:
    @classmethod
    async def download_repertoire(
        cls,
        cinema: str,
        cinemas_json_path: PurePath = Settings.CINEMAS_LIST_JSON_DEFAULT_PATH,
        date: date = date.today(),
    ) -> List[str] | None:
        """
        Download a repertoire for Cinema City (www.cinema-city.pl) for a specified branch and date

        :param cinema: name of a cinema venue for which you want to check the repertoire
        :param cinemas_json_path: path to json file containing cinemas and their respective IDs
        :param date: date for which to check the repertoire, defaults to today
        :return: a list of films names in the repertoire or None if the operation failed
        """
        cinema_id = cls._match_cinema_name_id(cinema, cinemas_json_path)

        try:
            session = HTMLSession()
            response_html = session.get(
                f"https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_id}&at={date}"
            )
            # render JS elements, required to get full page with films
            response_html.html.render()

            # from downloaded website select only elements containing films titles
            films = response_html.html.find(selector="h3.qb-movie-name")
            return [film.text for film in films]  # Convert HTML Elements into strings
        except requests.exceptions.ConnectionError:
            logger.error("No internet connection. Unable to fetch the repertoire")
            return None

    @classmethod
    async def download_cinemas_list(cls) -> dict:
        """
        Get all available cinemas with their respective IDs from www.cinema-city.pl
        and parse it to a dictionary.

        :return: a dictionary with <cinema_name>: <cinema_id> items
        """
        try:
            session = HTMLSession()
            response_html = session.get(
                "https://www.cinema-city.pl/#/buy-tickets-by-cinema", verify=False
            )
            # render JS elements, required to get full
            response_html.html.render()
            # from downloaded website select only elements containing films titles
            cinemas = response_html.html.find(selector="option[value][data-tokens]")

            # get a list of cinema names from the elements
            venues = [cinema.element.get("data-tokens") for cinema in cinemas]
            # get a list of cinema ids from the elements (needed to construct a valid url to get repertoire)
            ids = [int(cinema.element.get("value")) for cinema in cinemas]
            # make dictionary of venue name - id pairs
            return dict(zip(venues, ids))
        except requests.exceptions.ConnectionError:
            logger.error("No internet connection. Unable to fetch the list of cinemas.")
            return None
        # except requests.exceptions.ConnectTimeout:
        #     logging.error(msg="Internet connection slow or unstable. Unable to fetch the list of cinemas.")

    @classmethod
    def _update_cinemas_list(
        cls,
        updated_cinemas: dict,
        path: PurePath = Settings.CINEMAS_LIST_JSON_DEFAULT_PATH,
    ) -> None:
        """
        Get all available cinemas with their respective IDs from www.cinema-city.pl
        :updated_cinemas:
        :cinemas_json_path: a cinemas_json_path to json file to store a list of cinema venues in
        :return: None
        """
        _path = Path(path)

        # TODO: Add exception handling
        cinemas = {}
        with open(file=_path, mode="r", encoding="utf8") as f:
            try:
                cinemas = json.load(fp=f)
            except JSONDecodeError:
                logger.info("Missing json file with cinemas list. Populating content")

        with open(file=_path, mode="w", encoding="utf8") as f:
            try:
                cinemas["cinema_city"].update(updated_cinemas)
            except KeyError:
                cinemas["cinema_city"] = updated_cinemas
            json.dump(obj=cinemas, fp=f, ensure_ascii=False, indent=4)

    @classmethod
    def _match_cinema_name_id(
        cls, name: str, path: PurePath = Settings.CINEMAS_LIST_JSON_DEFAULT_PATH
    ) -> Optional[int]:
        """
        Returns id of a cinema specified by name. Based on cinema-list.json
        :param name: name of a cinema, case insensitive
        :return: id of a cinema or None if no match
        """
        _path = Path(path)
        # TODO: Add exception handling
        with open(file=_path, mode="a+", encoding="utf8") as f:
            cinema_city = json.load(f).get("cinema_city")
            for cinema, id in cinema_city.items():
                if re.search(name.lower(), cinema.lower()) is not None:
                    return id

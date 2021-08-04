import json
from json import JSONDecodeError
from pathlib import Path
from pprint import pprint
from typing import List, Optional, Union
from datetime import date
from requests_html import HTMLSession
import re

_json_default_path = '../cinemas-list.json'


def get_repertoire(cinema: str = 'manufaktura',
                   path: Union[str, Path] = _json_default_path,
                   repertoire_date: str = date.today()) -> Optional[List[str]]:
    """
    Get repertoire for a specified cinema and date from www.cinema-city.pl
    :param cinema: name of a cinema for which you want to check the repertoire
    :param path: path to json file containing cinemas and their respective IDs
    :param repertoire_date: date for which to check the repertoire, defaults to today
    :return: a list of films names in the repertoire
             or an empty list if the operation failed
             or if there are no films available in that date
    """
    cinema_id = _match_cinema_name_id(cinema, path)

    # TODO: Add exception handling
    session = HTMLSession()
    response_html = session.get(
        f'https://www.cinema-city.pl/#/buy-tickets-by-cinema?in-cinema={cinema_id}&at={repertoire_date}'
    )
    # render JS elements, required to get full page with films
    response_html.html.render()

    # from downloaded website select only elements containing films titles
    films = response_html.html.find(selector='h3.qb-movie-name')
    return [film.text for film in films]  # Convert HTML Elements into strings


def get_cinemas_list(path: Union[str, Path] = _json_default_path) -> Optional[dict[str, int]]:
    """
    Get all available cinemas with their respective IDs from a json file
    :return: dictionary with cinema name as a key and ID as a value
    """
    try:
        with open(path, 'r') as f:
            return json.load(f).get('cinema-city', default=None)
    except Union[JSONDecodeError, FileNotFoundError]:
        _update_cinemas_list()
        with open(path, 'r') as f:
            return json.load(f).get('cinema-city', default=None)


def _update_cinemas_list(path: Union[str, Path] = _json_default_path) -> None:
    """
    Get all available cinemas with their respective IDs from www.cinema-city.pl
    :return: None
    """
    # TODO: Add exception handling
    session = HTMLSession()
    response_html = session.get('https://www.cinema-city.pl/#/buy-tickets-by-cinema')
    # render JS elements, required to get full
    response_html.html.render()
    # from downloaded website select only elements containing films titles
    cinemas = response_html.html.find(selector='option[value][data-tokens]')

    # get a list of cinema names from the elements
    venues = [cinema.element.get('data-tokens') for cinema in cinemas]
    # get a list of cinema ids from the elements (needed to construct a valid url to get repertoire)
    ids = [int(cinema.element.get('value')) for cinema in cinemas]
    updated_cinemas = dict(zip(venues, ids))
    # TODO: Add exception handling
    with open(path, 'a+') as f:
        cinema_city = json.load(f).get('cinema-city', default=None)
        if cinema_city is not None:
            cinema_city['cinema-city'].update()


def _match_cinema_name_id(name: str, path: Union[str, Path] = _json_default_path) -> Optional[int]:
    """
    Returns id of a cinema specified by name. Based on cinema-list.json
    :param name: name of a cinema, case insensitive
    :return: id of a cinema or None if no match
    """
    # TODO: Add exception handling
    with open(path, 'a+') as f:
        cinema_city = json.load(f).get('cinema-city')
        for cinema, id in cinema_city.items():
            if re.search(name.lower(), cinema.lower()) is not None:
                return id
    return None


pprint(get_cinemas_list())
# pprint(get_repertoire())

import datetime
import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Union, Dict

import yaml


@dataclass
class Settings:
    config: Dict[str, str] = field(default_factory=dict)
    CINEMAS_LIST_JSON_DEFAULT_PATH = Path(
        'cinemas_list.json')  # default path to json with names and ids of cinemas
    SETTINGS_DEFAULT_PATH = Path('settings.yml')  # setting file with the defaults
    OUTPUT_DEFAULT_PATH = Path('repertoire.txt')  # default path to output file with the queried repertoire

    @classmethod
    def load_default_settings(cls, file: Union[str, Path] = SETTINGS_DEFAULT_PATH) -> None:
        """
        Load settings from settings.yml

        Settings:
        default_cinema_chain: one name from a list of supported cinema chains
        default_cinema_venue: one name from a list of venues of a selected cinema
        default_day: [today, tomorrow, <day of the week>]
        """
        try:
            cls.config = yaml.safe_load(open(file=file, mode='r', encoding='utf8'))
        except Exception as e:
            logging.error(e)

    @classmethod
    def resolve_date(cls, date_verbal: str) -> datetime.date:
        if date_verbal in ['today', 'dzisiaj']:
            return datetime.date.today()
        elif date_verbal in ['tomorrow', 'jutro']:
            return datetime.date.today() + datetime.timedelta(days=1)
        else:
            try:
                return datetime.datetime.strptime(date_verbal, '%d.%m.%Y').date()
            except ValueError as e:
                logging.error(e)

    @classmethod
    def validate_settings(cls, path: Union[str, Path] = SETTINGS_DEFAULT_PATH) -> bool:
        """
        A function to validate settings.yml file

        file encoding: must be utf-8 for Polish diacritical characters (ąężźśół) to be read correctly
        default_cinema_chain: must be one of: ['cinema_city, 'helios', 'multikino']
        default_cinema_venue: must be one of the keys in cinemas_list.json of a selected cinema chain

        :return: True if all the values are valid and encoding is correct, else return False
        """
        pass

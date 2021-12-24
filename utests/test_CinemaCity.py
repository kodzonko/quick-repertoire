import json
import os
from pathlib import Path

import pytest

import repertoire_parser.CinemaCity

test_json = Path(os.getcwd(), "Resources/cinemas-list-test.json")

test_json_backup = None


@pytest.fixture
def setup():
    with open(file=test_json, mode="r", encoding="utf8") as f:
        global test_json_backup
        test_json_backup = json.load(f)


def test_download_cinemas_list():
    cinemas = repertoire_parser.CinemaCity.download_cinemas_list()
    assert type(cinemas) is dict
    assert len(cinemas.items()) > 0
    assert cinemas


def test_update_cinemas_list():
    repertoire_parser.CinemaCity._update_cinemas_list(
        updated_cinemas={"test": 1}, path=test_json
    )
    with open(test_json, "r", encoding="utf8") as f:
        cinema_city_list = json.load(fp=f).get("cinema-city")
    assert cinema_city_list  # fails if no 'cinema-city' key in the json
    assert cinema_city_list.get("test")


def test_get_repertoire():
    assert True


@pytest.fixture
def teardown():
    with open(file=test_json, mode="w") as f:
        json.dump(obj=test_json_backup, fp=f, ensure_ascii=False, encoding="utf8")

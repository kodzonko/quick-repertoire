import json
import os
from os.path import exists

import pytest

import repertoire_parser.CinemaCity as cc

test_json = 'Resources/cinemas-list-test.json'
test_json_downloaded = 'Resources/cinemas-list-test-downloaded.json'


@pytest.fixture
def setup():
    pass


def test_download_cinemas_list():
    cinemas = cc._download_cinemas_list()
    assert type(cinemas) is dict
    assert len(cinemas.items()) > 0


def test_update_cinemas_list():
    cc._update_cinemas_list(test_json_downloaded)
    assert exists(test_json)
    with open(test_json, 'r') as f:
        cinema_city_list = json.load(fp=f).get('cinema-city')
    assert cinema_city_list  # fails if no 'cinema-city' key in the json


def test_get_repertoire():
    assert True


@pytest.fixture
def teardown():
    os.remove(test_json_downloaded)

import json
import os

import pytest

from repertoire_parser.CinemaCity import _download_cinemas_list, _update_cinemas_list

test_json = 'Resources/cinemas-list-test.json'
test_json_downloaded = 'Resources/cinemas-list-test-downloaded.json'

test_json_backup = None


@pytest.fixture
def setup():
    with open(file=test_json, mode='r') as f:
        global test_json_backup
        test_json_backup = json.load(f)


def test_download_cinemas_list():
    cinemas = _download_cinemas_list()
    assert type(cinemas) is dict
    assert len(cinemas.items()) > 0


def test_update_cinemas_list():
    _update_cinemas_list(updated_cinemas={}, path=test_json_downloaded)
    with open(test_json, 'r') as f:
        cinema_city_list = json.load(fp=f).get('cinema-city')
    assert cinema_city_list  # fails if no 'cinema-city' key in the json


def test_get_repertoire():
    assert True


@pytest.fixture
def teardown():
    with open(file=test_json, mode='w') as f:
        json.dump(obj=test_json_backup, fp=f)

    os.remove(test_json_downloaded)

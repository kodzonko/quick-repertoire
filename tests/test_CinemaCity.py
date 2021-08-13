import os
from os.path import exists
import pytest
from repertoire_parser.CinemaCity import get_cinemas_list, _update_cinemas_list

test_json = 'cinemas-list-test.json'


@pytest.fixture
def setup():
    pass


def test__update_cinemas_list():
    _update_cinemas_list(test_json)
    assert exists(test_json)


def test_get_repertoire():
    assert True


# def test_get_cinemas_list():


# def test__match_cinema_name_id():
#     assert False


@pytest.fixture
def teardown():
    os.remove(test_json)

import os
from os.path import exists
import pytest
from repertoire_parser.CinemaCity import get_cinemas_list, _update_cinemas_list

test_cache_folder = './test_cache'
test_json = os.path.join(test_cache_folder, 'cinemas-list-test.json')


@pytest.fixture
def setup():
    os.mkdir(test_cache_folder)


def test_get_repertoire():
    assert True


# def test_get_cinemas_list():
#     assert get_cinemas_list(test_json) is None


def test__update_cinemas_list():
    # _update_cinemas_list(test_json)
    # assert exists(test_json)
    assert True


def test__match_cinema_name_id():
    assert False


@pytest.fixture
def teardown():
    os.rmdir(test_cache_folder)

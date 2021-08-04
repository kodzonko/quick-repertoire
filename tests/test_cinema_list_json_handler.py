import os

import pytest

from repertoire_parser.CinemaCity import get_cinemas_list

test_cache_folder = './test_cache'
test_json = os.path.join(test_cache_folder, 'cinemas-list-test.json')


@pytest.fixture
def setup():
    os.mkdir(test_cache_folder)


def test_get_cinemas_list():
    assert get_cinemas_list(test_json) is None


@pytest.fixture
def teardown():
    os.rmdir(test_cache_folder)

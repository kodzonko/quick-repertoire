from typing import Any

import pytest
from mockito import mock
from pydantic_core import Url

from cinema_repertoire_analyzer.cinema_api.cinema import Cinema
from cinema_repertoire_analyzer.cinema_api.cinema_city import CinemaCity
from cinema_repertoire_analyzer.cinema_api.cinema_utils import (
    _get_cinema_class_by_cinema_chain,
    _get_cinema_settings_by_cinema_chain,
    cinema_factory,
)
from cinema_repertoire_analyzer.cinema_api.helios import Helios
from cinema_repertoire_analyzer.cinema_api.multikino import Multikino
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.settings import (
    CinemaCitySettings,
    CinemaSettings,
    HeliosSettings,
    MultikinoSettings,
    Settings,
)


@pytest.fixture
def cinema_city_settings() -> CinemaCitySettings:
    cinema_city_settings = mock(CinemaCitySettings)
    cinema_city_settings.REPERTOIRE_URL = Url(
        "https://www.cinema_city_repertoire_url_dummy_value.com/"
    )
    cinema_city_settings.VENUES_LIST_URL = Url(
        "https://www.cinema_city_venues_url_dummy_value.com/"
    )
    return cinema_city_settings  # type: ignore[no-any-return]


@pytest.fixture
def helios_settings() -> HeliosSettings:
    helios_settings = mock(HeliosSettings)
    helios_settings.REPERTOIRE_URL = Url("https://www.helios_repertoire_url_dummy_value.com/")
    helios_settings.VENUES_LIST_URL = Url("https://www.helios_venues_url_dummy_value.com/")
    return helios_settings  # type: ignore[no-any-return]


@pytest.fixture
def multikino_settings() -> MultikinoSettings:
    multikino_settings = mock(MultikinoSettings)
    multikino_settings.REPERTOIRE_URL = Url("https://www.multikino_repertoire_url_dummy_value.com/")
    multikino_settings.VENUES_LIST_URL = Url("https://www.multikino_venues_url_dummy_value.com/")
    return multikino_settings  # type: ignore[no-any-return]


@pytest.fixture
def all_settings(
    cinema_city_settings: CinemaCitySettings,
    helios_settings: HeliosSettings,
    multikino_settings: MultikinoSettings,
) -> Settings:
    settings_mock = mock(Settings)
    settings_mock.CINEMA_CITY_SETTINGS = cinema_city_settings
    settings_mock.HELIOS_SETTINGS = helios_settings
    settings_mock.MULTIKINO_SETTINGS = multikino_settings
    return settings_mock  # type: ignore[no-any-return]


@pytest.mark.parametrize(
    "cinema_chain, expected_cinema_class",
    [
        pytest.param(CinemaChain.CINEMA_CITY, CinemaCity),
        pytest.param(CinemaChain.HELIOS, Helios),
        pytest.param(CinemaChain.MULTIKINO, Multikino),
    ],
)
def test_get_cinema_class_by_cinema_chain_returns_matching_cinema_class(
    cinema_chain: CinemaChain, expected_cinema_class: type[Cinema]
) -> None:
    assert _get_cinema_class_by_cinema_chain(cinema_chain) == expected_cinema_class


@pytest.mark.parametrize(
    "cinema_chain, expected_settings",
    [
        pytest.param(CinemaChain.CINEMA_CITY, "cinema_city_settings"),
        pytest.param(CinemaChain.HELIOS, "helios_settings"),
        pytest.param(CinemaChain.MULTIKINO, "multikino_settings"),
    ],
)
def test_get_cinema_settings_by_cinema_chain_returns_matching_settings(
    all_settings: Settings,
    cinema_chain: CinemaChain,
    expected_settings: CinemaSettings,
    request: Any,
) -> None:
    assert (
        _get_cinema_settings_by_cinema_chain(cinema_chain, all_settings).__dict__
        == request.getfixturevalue(expected_settings).__dict__
    )


@pytest.mark.parametrize(
    "cinema_chain, expected_cinema",
    [
        pytest.param(
            CinemaChain.CINEMA_CITY,
            CinemaCity(
                Url("https://www.cinema_city_repertoire_url_dummy_value.com/"),
                Url("https://www.cinema_city_venues_url_dummy_value.com/"),
            ),
        ),
        pytest.param(
            CinemaChain.HELIOS,
            Helios(
                Url("https://www.helios_repertoire_url_dummy_value.com/"),
                Url("https://www.helios_venues_url_dummy_value.com/"),
            ),
        ),
        pytest.param(
            CinemaChain.MULTIKINO,
            Multikino(
                Url("https://www.multikino_repertoire_url_dummy_value.com/"),
                Url("https://www.multikino_venues_url_dummy_value.com/"),
            ),
        ),
    ],
)
def test_cinema_factory_constructs_correct_cinema_instance(
    all_settings: Settings, cinema_chain: CinemaChain, expected_cinema: Cinema
) -> None:
    assert cinema_factory(cinema_chain, all_settings).__dict__ == expected_cinema.__dict__

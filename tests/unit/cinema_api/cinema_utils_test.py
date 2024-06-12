import pytest
from mockito import mock

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
    cinema_city_settings.REPERTOIRE_URL = "cinema_city_repertoire_url_dummy_value"
    cinema_city_settings.VENUES_LIST_URL = "cinema_city_venues_url_dummy_value"
    return cinema_city_settings


@pytest.fixture
def helios_settings() -> HeliosSettings:
    helios_settings = mock(HeliosSettings)
    helios_settings.REPERTOIRE_URL = "helios_repertoire_url_dummy_value"
    helios_settings.VENUES_LIST_URL = "helios_venues_url_dummy_value"
    return helios_settings


@pytest.fixture
def multikino_settings() -> MultikinoSettings:
    multikino_settings = mock(MultikinoSettings)
    multikino_settings.REPERTOIRE_URL = "multikino_repertoire_url_dummy_value"
    multikino_settings.VENUES_LIST_URL = "multikino_venues_url_dummy_value"
    return multikino_settings


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
    return settings_mock


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
    all_settings: Settings, cinema_chain: CinemaChain, expected_settings: CinemaSettings, request
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
                "cinema_city_repertoire_url_dummy_value", "cinema_city_venues_url_dummy_value"
            ),
        ),
        pytest.param(
            CinemaChain.HELIOS,
            Helios("helios_repertoire_url_dummy_value", "helios_venues_url_dummy_value"),
        ),
        pytest.param(
            CinemaChain.MULTIKINO,
            Multikino("multikino_repertoire_url_dummy_value", "multikino_venues_url_dummy_value"),
        ),
    ],
)
def test_cinema_factory_constructs_correct_cinema_instanxce(
    all_settings: Settings, cinema_chain: CinemaChain, expected_cinema: Cinema
) -> None:
    assert cinema_factory(cinema_chain, all_settings).__dict__ == expected_cinema.__dict__

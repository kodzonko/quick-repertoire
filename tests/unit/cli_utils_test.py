from datetime import datetime, timedelta

import pytest
import typer
from mockito import mock

from cinema_repertoire_analyzer.cli_utils import (
    cinema_input_parser,
    cinema_venue_input_parser,
    date_input_parser,
    _venue_results_to_table_title,
)
from cinema_repertoire_analyzer.database.models import (
    CinemaVenuesBase,
    CinemaCityVenues,
    MultikinoVenues,
    HeliosVenues,
)
from cinema_repertoire_analyzer.enums import CinemaChain


@pytest.mark.unit
@pytest.mark.parametrize(
    "input_, output",
    [
        pytest.param("Cinema City", CinemaChain.CINEMA_CITY),
        pytest.param("cinema-city", CinemaChain.CINEMA_CITY),
        pytest.param("cinema_city", CinemaChain.CINEMA_CITY),
        pytest.param("multikino", CinemaChain.MULTIKINO),
        pytest.param("Helios", CinemaChain.HELIOS),
    ],
)
def test_cinema_input_parser_parses_user_input_correctly(input_: str, output: CinemaChain) -> None:
    assert cinema_input_parser(input_) == output


@pytest.mark.unit
def test_cinema_input_parser_raises_error_on_unrecognized_input() -> None:
    with pytest.raises(
        typer.BadParameter,
        match='Kino "foo" nie jest wspierane. Wybierz jedno z: Cinema City, Helios, Multikino',
    ):
        cinema_input_parser("foo")


@pytest.mark.unit
@pytest.mark.parametrize(
    "input_, output",
    [
        pytest.param(" Manufaktura ", "%Manufaktura%"),
        pytest.param("warszawa janki", "%warszawa%janki%"),
        pytest.param("wrocłavia.", "%wroc_avia%"),
        pytest.param("multikino", "%multikino%"),
        pytest.param("Helios\n", "%Helios%"),
    ],
)
def test_cinema_venue_input_parser_parses_user_input_correctly(
    input_: str, output: CinemaChain
) -> None:
    assert cinema_venue_input_parser(input_) == output


@pytest.mark.unit
@pytest.mark.parametrize(
    "input_, output",
    [
        pytest.param("dziś", datetime.now().strftime("%Y-%m-%d")),
        pytest.param("dzis", datetime.now().strftime("%Y-%m-%d")),
        pytest.param("dzisiaj", datetime.now().strftime("%Y-%m-%d")),
        pytest.param("today", datetime.now().strftime("%Y-%m-%d")),
        pytest.param("jutro", (datetime.now() + timedelta(days=1)).strftime("%Y-%m-%d")),
        pytest.param("tomorrow", (datetime.now() + timedelta(days=1)).strftime("%Y-%m-%d")),
        pytest.param("2021-12-31", "2021-12-31"),
    ],
)
def test_date_input_parser_parses_user_input_correctly(input_: str, output: str) -> None:
    assert date_input_parser(input_) == output


@pytest.mark.unit
def test_date_input_parser_raises_error_on_unrecognized_input() -> None:
    with pytest.raises(
        typer.BadParameter,
        match="Data: foo nie jest we wspieranym formacie: YYYY-MM-DD | dzis | jutro | itp...",
    ):
        date_input_parser("foo")


@pytest.mark.unit
@pytest.mark.parametrize(
    "input_, output",
    [
        pytest.param(CinemaCityVenues, "Znalezione lokale sieci Cinema City"),
        pytest.param(MultikinoVenues, "Znalezione lokale sieci Multikino"),
        pytest.param(HeliosVenues, "Znalezione lokale sieci Helios"),
    ],
)
def test_venue_results_to_table_title_converts_venue_results_to_table_title_correctly(
    input_: CinemaVenuesBase, output: str
) -> None:
    assert _venue_results_to_table_title(input_) == output

from datetime import datetime, timedelta

import pytest
import typer

from cinema_repertoire_analyzer.cli_utils import cinema_venue_input_parser, date_input_parser
from cinema_repertoire_analyzer.database.models import CinemaVenues


@pytest.mark.unit
@pytest.mark.parametrize(
    "input_, output",
    [
        pytest.param(" Manufaktura ", "%Manufaktura%"),
        pytest.param("warszawa janki", "%warszawa%janki%"),
        pytest.param("wrocłavia.", "%wroc_avia%"),
    ],
)
def test_cinema_venue_input_parser_parses_user_input_correctly(
    input_: str, output: CinemaVenues
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

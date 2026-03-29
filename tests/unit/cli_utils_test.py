from datetime import datetime, timedelta

import pytest
import typer
from rich.console import Console

from cinema_repertoire_analyzer.cinema_api.models import (
    CinemaVenue,
    MoviePlayDetails,
    Repertoire,
    RepertoireCliTableMetadata,
)
from cinema_repertoire_analyzer.cli_utils import (
    cinema_venue_input_parser,
    date_input_parser,
    db_venues_to_cli,
    repertoire_to_cli,
)
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails


@pytest.mark.unit
@pytest.mark.parametrize(
    ("input_", "output"),
    [
        pytest.param(" Manufaktura ", "%Manufaktura%"),
        pytest.param("warszawa janki", "%warszawa%janki%"),
        pytest.param("wroclawia.", "%wroclawia%"),
    ],
)
def test_cinema_venue_input_parser_parses_user_input_correctly(input_: str, output: str) -> None:
    assert cinema_venue_input_parser(input_) == output


@pytest.mark.unit
@pytest.mark.parametrize(
    ("input_", "output"),
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
def test_db_venues_to_cli_prints_empty_state_for_missing_venues() -> None:
    console = Console(record=True, width=120)

    db_venues_to_cli([], "Cinema City", console)

    assert "Brak kin tej sieci w bazie danych." in console.export_text()


@pytest.mark.unit
def test_db_venues_to_cli_renders_table_for_found_venues() -> None:
    console = Console(record=True, width=200)
    venues = [
        CinemaVenue(chain_id="cinema-city", venue_name="Lodz - Manufaktura", venue_id="1080"),
        CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="1097"),
    ]

    db_venues_to_cli(venues, "Cinema City", console)

    rendered_output = console.export_text()
    assert "Znalezione lokale sieci Cinema" in rendered_output
    assert "City" in rendered_output
    assert "venue_name" in rendered_output
    assert "Wroclaw - Wroclavia" in rendered_output


@pytest.mark.unit
def test_repertoire_to_cli_prints_empty_state_for_missing_movies() -> None:
    console = Console(record=True, width=120)
    metadata = RepertoireCliTableMetadata(
        chain_display_name="Cinema City",
        repertoire_date="2024-06-01",
        cinema_venue_name="Wroclaw - Wroclavia",
    )

    repertoire_to_cli([], metadata, {}, console)

    assert "Brak repertuaru" in console.export_text()


@pytest.mark.unit
def test_repertoire_to_cli_renders_ratings_when_available() -> None:
    console = Console(record=True, width=200)
    metadata = RepertoireCliTableMetadata(
        chain_display_name="Cinema City",
        repertoire_date="2024-06-01",
        cinema_venue_name="Wroclaw - Wroclavia",
    )
    repertoire = [
        Repertoire(
            title="Test Movie",
            genres="Thriller",
            play_length="120 min",
            original_language="EN",
            play_details=[
                MoviePlayDetails(
                    format="2D", play_language="NAP: PL", play_times=["10:00", "12:30"]
                )
            ],
        )
    ]
    ratings = {
        "Test Movie": TmdbMovieDetails(rating="8.5/10\n(glosy: 2000)", summary="A tense mystery.")
    }

    repertoire_to_cli(repertoire, metadata, ratings, console)

    rendered_output = console.export_text()
    assert "Repertuar dla Cinema City (Wroclaw - Wroclavia)" in rendered_output
    assert "Ocena z TMDB" in rendered_output
    assert "8.5/10" in rendered_output
    assert "A tense mystery." in rendered_output

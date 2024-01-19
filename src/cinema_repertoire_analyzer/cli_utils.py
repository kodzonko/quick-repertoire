from datetime import datetime, timedelta

import typer
from rich.console import Console
from rich.table import Table

from cinema_repertoire_analyzer.database.models import CinemaVenuesBase
from cinema_repertoire_analyzer.enums import CinemaChain


def cinema_input_parser(cinema_name: str) -> CinemaChain:
    """Parse cinema name input to match the format of the config file."""
    try:
        return CinemaChain[cinema_name.upper().replace(" ", "_").replace("-", "_")]
    except KeyError:
        raise typer.BadParameter(
            f'"{cinema_name}" is not supported. Please choose one of the following: '
            f'{", ".join(CinemaChain.__members__.values())}'
        )


def cinema_venue_input_parser(cinema_venue: str) -> str:
    """Parse cinema venue input to match the format of the config file."""


def date_input_parser(date: str) -> str:
    """Parse date input to match the expected format.

    Returns:
        Date in the format YYYY-MM-DD.
    """
    if date == "today":
        return datetime.now().strftime("%Y-%m-%d")
    if date == "tomorrow":
        return (datetime.now() + timedelta(days=1)).strftime("%Y-%m-%d")
    try:
        # If date is given, verify if it's in the expected format
        datetime.strptime(date, "%Y-%m-%d")
        return date
    except ValueError:
        raise typer.BadParameter(
            f"Date {date} is not in the expected format: YYYY-MM-DD | today | tomorrow"
        )


def _venue_results_to_table_title(venue: CinemaVenuesBase) -> str:
    """Convert venue results to a table title."""
    venues_class_to_table_title_mapping = {
        "cinema_city_venues": "Cinema City",
        "multikino_venues": "Multikino",
        "helios_venues": "Helios",
    }
    return f"Kina sieci {venues_class_to_table_title_mapping[venue.__table__.name]}"


def db_venues_to_cli(venues: list[CinemaVenuesBase], sink: Console) -> str:
    """Convert list of venues to a string."""
    if not venues:
        sink.print("Brak kin tej sieci w bazie danych.")
        return
    table = Table(title=_venue_results_to_table_title(venues[0]))
    for column in venues[0].__table__.columns:
        table.add_column(column.name)
    for venue in venues:
        table.add_row(*venue.list_values())

    sink.print(table)

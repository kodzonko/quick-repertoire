import re
from datetime import datetime, timedelta

import rich
import typer
from rich.table import Table

from cinema_repertoire_analyzer.cinema_api.models import Repertoire, RepertoireCliTableMetadata
from cinema_repertoire_analyzer.database.models import CinemaVenues
from cinema_repertoire_analyzer.ratings_api.models import TmdbMovieDetails


def cinema_venue_input_parser(cinema_venue: str) -> str:
    """Parse cinema venue input to prepare it for querying the db in a permissive way."""
    trimmed_outer_whitespaces = cinema_venue.strip()
    non_letters_removed = re.sub(r"\W", " ", trimmed_outer_whitespaces)
    whitespaces_trimmed = re.sub(r"\s+", ",", non_letters_removed)
    nonascii_removed = re.sub(r"[^\x00-\x7F]", "_", whitespaces_trimmed)
    surrounding_wildcards_added = f"%{nonascii_removed.replace(",", "%")}%"
    multiple_consecutive_wildcards_replaced = re.sub("%{2,}", "%", surrounding_wildcards_added)
    return multiple_consecutive_wildcards_replaced


def date_input_parser(date: str) -> str:
    """Parse date input to match the expected format.

    Returns:
        Date in the format YYYY-MM-DD.
    """
    if date in {"dziś", "dzis", "dzisiaj", "today"}:
        return datetime.now().strftime("%Y-%m-%d")
    if date in {"jutro", "tomorrow"}:
        return (datetime.now() + timedelta(days=1)).strftime("%Y-%m-%d")
    try:
        # If date is given, verify if it's in the expected format
        datetime.strptime(date, "%Y-%m-%d")
        return date
    except ValueError:
        raise typer.BadParameter(
            f"Data: {date} nie jest we wspieranym formacie: YYYY-MM-DD | dzis | jutro | itp..."
        )


def db_venues_to_cli(venues: list[CinemaVenues], sink: rich.console.Console) -> None:
    """Print cinema venues as a pretty-printed table in a console."""
    if not venues:
        sink.print("Brak kin tej sieci w bazie danych.")
        return
    table = Table(title="Znalezione lokale sieci Cinema City")
    for column in venues[0].__table__.columns:
        table.add_column(column.name)
    for venue in venues:
        table.add_row(*venue.list_values())

    sink.print(table)


def repertoire_to_cli(
    repertoire: list[Repertoire],
    table_metadata: RepertoireCliTableMetadata,
    ratings: dict[str, TmdbMovieDetails],
    sink: rich.console.Console,
) -> None:
    """Print a repertoire as a pretty-printed table in a console."""
    if not repertoire:
        sink.print("Brak repertuaru do wyświetlenia.")
        return
    table = Table(
        title=f"Repertuar dla Cinema City "
        f"({table_metadata.cinema_venue_name}) na dzień: {table_metadata.repertoire_date}",
        show_lines=True,
        header_style="bold",
    )
    table.add_column("Tytuł", max_width=20)
    table.add_column("Gatunki", max_width=15)
    table.add_column("Długość")
    table.add_column("Język\noryg.")
    table.add_column("Seanse", max_width=20)
    if ratings:
        table.add_column("Ocena z TMDB")
        table.add_column("Opis z TMDB")
    for movie in repertoire:
        row_content = [
            movie.title,
            movie.genres,
            movie.play_length,
            movie.original_language,
            "\n".join(
                [
                    f"[{play.format}, {play.play_language}]:\n{" ".join(play.play_times)}"
                    for play in movie.play_details
                ]
            ),
        ]
        if ratings:
            row_content.append(ratings[movie.title].rating)
            row_content.append(ratings[movie.title].summary)

        table.add_row(*row_content)
    sink.print(table)

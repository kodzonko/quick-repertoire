from datetime import datetime
from sqlite3 import Error

import pytest
import sqlalchemy
from mockito import args, mock, when
from sqlalchemy.orm import Session
from sqlalchemy.orm.query import Query, RowReturningQuery

from database.database_manager import DatabaseManager
from database.models import CinemaVenues
from enums import CinemaChain
from exceptions import DBConnectionError


@pytest.fixture
def db_manager() -> DatabaseManager:
    return DatabaseManager()


@pytest.fixture
def engine() -> sqlalchemy.Engine:
    return mock(sqlalchemy.Engine)


@pytest.fixture
def session() -> Session:
    return mock(Session)


@pytest.fixture
def query() -> Query:
    return mock(Query)


@pytest.fixture
def row_returning_query() -> RowReturningQuery:
    return mock(RowReturningQuery)


def test_get_cinema_venues_returns_venues_without_city_filter(
    db_manager: DatabaseManager,
    session: Session,
    row_returning_query: RowReturningQuery,
) -> None:
    expected = ["Cinema 1", "Cinema 2"]

    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(session).query(CinemaVenues.venue_name).thenReturn(row_returning_query)
    when(row_returning_query).filter(...).thenReturn(row_returning_query)
    when(row_returning_query).group_by(CinemaVenues.city).thenReturn(
        row_returning_query
    )
    when(row_returning_query).all().thenReturn([("Cinema 1",), ("Cinema 2",)])

    assert db_manager.get_cinema_venues(CinemaChain.CINEMA_CITY) == expected


def test_get_cinema_venues_returns_venues_with_city_filter(
    db_manager: DatabaseManager,
    session: Session,
    row_returning_query: RowReturningQuery,
) -> None:
    expected = ["Cinema 1", "Cinema 2"]

    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(session).query(CinemaVenues.venue_name).thenReturn(row_returning_query)
    when(row_returning_query).filter(...).thenReturn(row_returning_query)
    when(row_returning_query).group_by(CinemaVenues.city).thenReturn(
        row_returning_query
    )
    when(row_returning_query).all().thenReturn([("Cinema 1",), ("Cinema 2",)])

    assert (
        db_manager.get_cinema_venues(CinemaChain.CINEMA_CITY, "some city") == expected
    )


def test_update_cinema_venues_inserts_records_to_db(
    db_manager: DatabaseManager, session: Session, query: Query
) -> None:
    venues = ["Cinema Venue 1", "Cinema Venue 1"]

    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(session).query(CinemaVenues).thenReturn(query)
    when(query).delete()
    when(session).add_all(venues)

    db_manager.update_cinema_venues(venues)


def test_get_repertoire_returns_repertoire(
    db_manager: DatabaseManager,
    session: Session,
    row_returning_query: RowReturningQuery,
) -> None:
    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(session).query(...).thenReturn(row_returning_query)
    when(row_returning_query).filter(...).thenReturn(row_returning_query)
    when(row_returning_query).all().thenReturn(
        [("Repertoire Entry 1",), ("Repertoire Entry  2",)]
    )
    db_manager.get_repertoire(
        datetime(2022, 1, 1),
        CinemaChain.CINEMA_CITY,
        venue="some venue",
        city="some city",
        format="some format",
        language="some language",
    )


def test_database_manager_fails_to_create_instance_due_to_error() -> None:
    when(sqlalchemy).create_engine("sqlite:///some path").thenRaise(
        Error("some connection error")
    )

    with pytest.raises(DBConnectionError, match="Failed to connect with the database."):
        DatabaseManager("some path")

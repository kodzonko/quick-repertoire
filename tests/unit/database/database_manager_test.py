from sqlite3 import Error

import pytest
import sqlalchemy
from mockito import args, mock, when
from sqlalchemy.orm import Session
from sqlalchemy.orm.query import Query, RowReturningQuery

import cinema_repertoire_analyzer.database.db_utils as db_utils
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.database.models import CinemaCityVenues
from cinema_repertoire_analyzer.enums import CinemaChain
from cinema_repertoire_analyzer.exceptions import DBConnectionError


@pytest.fixture
def db_manager() -> DatabaseManager:
    return DatabaseManager("sqlite:///some path")


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


@pytest.fixture
def cinema_venues() -> list[CinemaCityVenues]:
    return [mock(CinemaCityVenues), mock(CinemaCityVenues)]


@pytest.fixture
def cinema_venue() -> CinemaCityVenues:
    return mock(CinemaCityVenues)


def test_update_cinema_venues_inserts_records_to_db(
    db_manager: DatabaseManager,
    session: Session,
    query: Query,
    cinema_venues: list[CinemaCityVenues],
) -> None:
    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(db_utils).get_table_by_cinema_chain(CinemaChain.CINEMA_CITY).thenReturn(CinemaCityVenues)
    when(session).query(CinemaCityVenues).thenReturn(query)
    when(query).delete()
    when(session).add_all(cinema_venues)

    db_manager.update_cinema_venues(CinemaChain.CINEMA_CITY, cinema_venues)


def test_database_manager_fails_to_create_instance_due_to_error() -> None:
    when(sqlalchemy).create_engine("sqlite:///some path").thenRaise(Error("some connection error"))

    with pytest.raises(DBConnectionError, match="Failed to connect with the database."):
        DatabaseManager("some path")


def test_get_venue_by_name_returns_venue(
    db_manager: DatabaseManager,
    session: Session,
    row_returning_query: RowReturningQuery,
    cinema_venue: CinemaCityVenues,
) -> None:
    when(session).__enter__().thenReturn(session)
    when(session).__exit__(*args)
    when(db_manager)._session_constructor().thenReturn(session)
    when(session).query(CinemaCityVenues).thenReturn(row_returning_query)
    when(row_returning_query).filter_by(venue_name="some name").thenReturn(row_returning_query)
    when(row_returning_query).one().thenReturn(cinema_venue)

    assert db_manager.get_venue_by_venue_name(CinemaChain.CINEMA_CITY, "some name") == cinema_venue

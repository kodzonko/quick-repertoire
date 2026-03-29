from pathlib import Path
from sqlite3 import Error

import pytest
import sqlalchemy
from mockito import mock, when

from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.database.models import Base, CinemaVenues
from cinema_repertoire_analyzer.exceptions import DatabaseConnectionError


@pytest.fixture
def db_path(tmp_path: Path) -> Path:
    return tmp_path / "test_db.sqlite"


@pytest.fixture
def db_manager(db_path: Path) -> DatabaseManager:
    return DatabaseManager(db_path)


@pytest.mark.unit
def test_database_manager_bootstraps_schema_on_init(db_path: Path) -> None:
    db_manager = DatabaseManager(db_path)

    assert db_manager.get_all_venues() == []
    assert db_path.exists() is True


@pytest.mark.unit
def test_update_cinema_venues_replaces_existing_records(db_manager: DatabaseManager) -> None:
    venues = [
        CinemaVenues(venue_name="Warszawa - Janki", venue_id="1"),
        CinemaVenues(venue_name="Wrocław - Wroclavia", venue_id="2"),
    ]

    db_manager.update_cinema_venues(venues)

    assert [venue.venue_name for venue in db_manager.get_all_venues()] == [
        "Warszawa - Janki",
        "Wrocław - Wroclavia",
    ]


@pytest.mark.unit
def test_find_venues_by_name_returns_matching_venues(db_manager: DatabaseManager) -> None:
    db_manager.update_cinema_venues(
        [
            CinemaVenues(venue_name="Warszawa - Janki", venue_id="1"),
            CinemaVenues(venue_name="Warszawa - Arkadia", venue_id="2"),
            CinemaVenues(venue_name="Wrocław - Wroclavia", venue_id="3"),
        ]
    )

    found_venues = db_manager.find_venues_by_name("Warszawa")

    assert [venue.venue_name for venue in found_venues] == [
        "Warszawa - Janki",
        "Warszawa - Arkadia",
    ]


@pytest.mark.unit
def test_database_manager_raises_domain_error_when_engine_creation_fails(db_path: Path) -> None:
    engine_factory = mock()
    when(sqlalchemy).create_engine(f"sqlite:///{db_path}").thenRaise(Error("boom"))
    when(sqlalchemy.orm).sessionmaker(engine_factory).thenReturn(mock())

    with pytest.raises(DatabaseConnectionError):
        DatabaseManager(db_path)


@pytest.mark.unit
def test_database_manager_raises_domain_error_when_schema_bootstrap_fails(db_path: Path) -> None:
    engine = mock()
    metadata = mock()
    original_metadata = Base.metadata
    Base.metadata = metadata
    when(sqlalchemy).create_engine(f"sqlite:///{db_path}").thenReturn(engine)
    when(metadata).create_all(engine).thenRaise(sqlalchemy.exc.SQLAlchemyError("boom"))

    try:
        with pytest.raises(DatabaseConnectionError):
            DatabaseManager(db_path)
    finally:
        Base.metadata = original_metadata


@pytest.mark.unit
def test_database_manager_calls_create_all_on_init(db_path: Path) -> None:
    engine = mock()
    session_factory = mock()
    metadata = mock()
    original_metadata = Base.metadata
    Base.metadata = metadata
    when(sqlalchemy).create_engine(f"sqlite:///{db_path}").thenReturn(engine)
    when(metadata).create_all(engine)
    when(sqlalchemy.orm).sessionmaker(engine).thenReturn(session_factory)

    try:
        manager = DatabaseManager(db_path)
    finally:
        Base.metadata = original_metadata

    assert manager._session_constructor is session_factory

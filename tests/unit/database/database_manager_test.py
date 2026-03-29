from pathlib import Path
from sqlite3 import Error

import pytest
import sqlalchemy
from mockito import mock, when

from cinema_repertoire_analyzer.cinema_api.models import CinemaChainId, CinemaVenue
from cinema_repertoire_analyzer.database.database_manager import DatabaseManager
from cinema_repertoire_analyzer.database.models import Base
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

    assert db_manager.get_all_venues(CinemaChainId.CINEMA_CITY) == []
    assert db_path.exists() is True


@pytest.mark.unit
def test_replace_venues_replaces_existing_records_only_for_selected_chain(
    db_manager: DatabaseManager,
) -> None:
    db_manager.replace_venues(
        "cinema-city",
        [CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="1")],
    )
    db_manager.replace_venues(
        "helios", [CinemaVenue(chain_id="helios", venue_name="Lodz - Sukcesja", venue_id="2")]
    )

    db_manager.replace_venues(
        "cinema-city",
        [CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="3")],
    )

    assert [venue.venue_name for venue in db_manager.get_all_venues("cinema-city")] == [
        "Wroclaw - Wroclavia"
    ]
    assert [venue.venue_name for venue in db_manager.get_all_venues("helios")] == [
        "Lodz - Sukcesja"
    ]


@pytest.mark.unit
def test_find_venues_by_name_returns_matching_venues_for_selected_chain(
    db_manager: DatabaseManager,
) -> None:
    db_manager.replace_venues(
        "cinema-city",
        [
            CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Janki", venue_id="1"),
            CinemaVenue(chain_id="cinema-city", venue_name="Warszawa - Arkadia", venue_id="2"),
            CinemaVenue(chain_id="cinema-city", venue_name="Wroclaw - Wroclavia", venue_id="3"),
        ],
    )
    db_manager.replace_venues(
        "helios", [CinemaVenue(chain_id="helios", venue_name="Warszawa - Blue City", venue_id="4")]
    )

    found_venues = db_manager.find_venues_by_name("cinema-city", "Warszawa")

    assert [venue.venue_name for venue in found_venues] == [
        "Warszawa - Arkadia",
        "Warszawa - Janki",
    ]


@pytest.mark.unit
def test_replace_venues_batch_replaces_all_selected_chains_transactionally(
    db_manager: DatabaseManager,
) -> None:
    db_manager.replace_venues(
        "cinema-city", [CinemaVenue(chain_id="cinema-city", venue_name="Old City", venue_id="1")]
    )
    db_manager.replace_venues(
        "helios", [CinemaVenue(chain_id="helios", venue_name="Old Helios", venue_id="2")]
    )

    db_manager.replace_venues_batch(
        {
            "cinema-city": [
                CinemaVenue(chain_id="cinema-city", venue_name="New City", venue_id="3")
            ],
            "helios": [CinemaVenue(chain_id="helios", venue_name="New Helios", venue_id="4")],
        }
    )

    assert [venue.venue_name for venue in db_manager.get_all_venues("cinema-city")] == ["New City"]
    assert [venue.venue_name for venue in db_manager.get_all_venues("helios")] == ["New Helios"]


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

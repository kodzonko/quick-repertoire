from sqlalchemy import TEXT, Column
from sqlalchemy.orm import declarative_base

Base = declarative_base()


class CinemaVenuesBase(Base):
    """Base class for cinema venues models."""

    __abstract__ = True

    venue_name = Column(TEXT, primary_key=True)

    def list_values(self) -> list:
        """Return list of values for a single venue."""
        return [getattr(self, column.name) for column in self.__table__.columns]


class CinemaCityVenues(CinemaVenuesBase):
    """Model for Cinema City venues."""

    __tablename__ = "cinema_city_venues"
    venue_id = Column(TEXT, unique=True)


class HeliosVenues(CinemaVenuesBase):
    """Model for Helios venues."""

    __tablename__ = "helios_venues"
    venue_id = Column(TEXT, unique=True)


class MultikinoVenues(CinemaVenuesBase):
    """Model for Multikino venues."""

    __tablename__ = "multikino_venues"

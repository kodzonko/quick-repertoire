from sqlalchemy import TEXT, Column
from sqlalchemy.orm import declarative_base

Base = declarative_base()


class CinemaVenuesBase(Base):
    __abstract__ = True

    venue_name = Column(TEXT, primary_key=True)

    def list_values(self) -> list:
        """Return list of values for a single venue."""
        return [getattr(self, column.name) for column in self.__table__.columns]


class CinemaCityVenues(CinemaVenuesBase):
    __tablename__ = "cinema_city_venues"
    venue_id = Column(TEXT, unique=True)


class HeliosVenues(CinemaVenuesBase):
    __tablename__ = "helios_venues"
    venue_id = Column(TEXT, unique=True)


class MultikinoVenues(CinemaVenuesBase):
    __tablename__ = "multikino_venues"

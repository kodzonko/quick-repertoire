from sqlalchemy import TEXT, Column
from sqlalchemy.orm import declarative_base

Base = declarative_base()


class CinemaVenues(Base):
    """Model for Cinema City venues."""

    __tablename__ = "cinema_city_venues"
    venue_name = Column(TEXT, primary_key=True)
    venue_id = Column(TEXT, unique=True)

    def list_values(self) -> list:
        """Return list of values for a single venue."""
        return [getattr(self, column.name) for column in self.__table__.columns]

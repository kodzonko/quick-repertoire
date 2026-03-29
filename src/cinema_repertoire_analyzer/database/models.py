from sqlalchemy import TEXT, Column
from sqlalchemy.orm import declarative_base

from cinema_repertoire_analyzer.cinema_api.models import CinemaVenue

Base = declarative_base()


class CinemaVenueRecord(Base):
    """ORM model for cached cinema venues across chains."""

    __tablename__ = "cinema_venues"
    chain_id = Column(TEXT, primary_key=True)
    venue_id = Column(TEXT, primary_key=True)
    venue_name = Column(TEXT, nullable=False)

    def to_domain(self) -> CinemaVenue:
        """Convert the ORM record into the shared venue model."""
        return CinemaVenue(
            chain_id=str(self.chain_id),
            venue_id=str(self.venue_id),
            venue_name=str(self.venue_name),
        )

    @classmethod
    def from_domain(cls, venue: CinemaVenue) -> CinemaVenueRecord:
        """Create an ORM record from the shared venue model."""
        return cls(chain_id=venue.chain_id, venue_id=venue.venue_id, venue_name=venue.venue_name)

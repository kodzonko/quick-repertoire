from sqlalchemy import (
    DATETIME,
    INTEGER,
    REAL,
    TEXT,
    Column,
    ForeignKey,
    UniqueConstraint,
)
from sqlalchemy.orm import declarative_base

Base = declarative_base()


class CinemaVenues(Base):
    __tablename__ = "cinema_venues"
    cinema_id = Column(INTEGER, primary_key=True)
    venue_name = Column(TEXT)
    cinema_chain = Column(TEXT)
    venue_id = Column(INTEGER)
    city = Column(TEXT)
    UniqueConstraint("venue_name", "cinema_chain", sqlite_on_conflict="IGNORE")


class Movies(Base):
    __tablename__ = "movies"
    movie_id = Column(INTEGER, primary_key=True)
    title = Column(TEXT)
    genres = Column(TEXT)
    description = Column(TEXT)
    imdb_rating = Column(REAL)
    filmweb_rating = Column(REAL)
    imdb_url = Column(TEXT)
    filmweb_url = Column(TEXT)


class Repertoire(Base):
    __tablename__ = "repertoire"
    repertoire_id = Column(INTEGER, primary_key=True)
    play_time = Column(DATETIME)
    play_cinema_id = Column(INTEGER, ForeignKey("cinema_venues.cinema_id"))
    movie_id = Column(INTEGER, ForeignKey("movies.movie_id"))
    movie_format = Column(TEXT)
    movie_language = Column(TEXT)
    UniqueConstraint(
        "play_time", "play_cinema_id", "movie_id", sqlite_on_conflict="IGNORE"
    )

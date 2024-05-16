from pydantic import BaseModel


class TmdbMovieDetails(BaseModel):
    """Details about a movie from the TMDB API."""

    rating: str
    summary: str

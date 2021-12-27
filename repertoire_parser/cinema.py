from abc import ABC, abstractmethod
from datetime import date
from pathlib import PurePath
from typing import List


class Cinema(ABC):
    @classmethod
    @abstractmethod
    async def download_repertoire(
        cls,
        cinema: str,
        cinemas_json_path: PurePath,
        date: date,
    ) -> List[str] | None:
        pass

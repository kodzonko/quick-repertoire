from datetime import date
from pathlib import PurePath
from typing import List

from repertoire_parser.cinema import Cinema


class Multikino(Cinema):
    @classmethod
    async def download_repertoire(
            cls, cinema: str, cinemas_json_path: PurePath, date: date
    ) -> List[str] | None:
        pass

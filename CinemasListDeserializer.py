from dataclasses import dataclass
from dataclasses_json import dataclass_json


@dataclass_json
@dataclass
class CinemasList:
    cinema_city: dict
    helios: dict
    multikino: dict

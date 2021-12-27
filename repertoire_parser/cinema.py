from abc import ABC, abstractmethod


class ICinema(ABC):
    @classmethod
    @abstractmethod
    def download_repertoire(cls):
        pass

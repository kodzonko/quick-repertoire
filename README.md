<img src=https://github.com/kodzonko/cinema-repertoire-analyzer/blob/master/docs/demo.gif/ width="80%">

# Cinema Repertoire Analyzer

![Coveralls](https://img.shields.io/coverallsCoverage/github/kodzonko/cinema-repertoire-analyzer) ![Python Version from PEP 621 TOML](https://img.shields.io/python/required-version-toml?tomlFilePath=https%3A%2F%2Fraw.githubusercontent.com%2Fkodzonko%2Fcinema-repertoire-analyzer%2Fmaster%2Fpyproject.toml) ![GitHub last commit](https://img.shields.io/github/last-commit/kodzonko/cinema-repertoire-analyzer)

Scraper repertuarów dla Cinema City, umożliwia zaciągnięcie repertuaru na najbliższe dni dla wybranego kina sieci Cinema City.
Wyniki pokazane wraz z oceną i streszczeniem filmu z TMDB.

Datę (w tym wartości takie jak "dzis", "jutro" oraz nazwę kina można podać w parametrach lub ustawić domyślne wartości w pliku `run.env`.)

## Wymagania

- poetry
- python 3.12
- (opcjonalnie) make

## Instalacja

```shell
$ make install
lub
$ poetry install
$ poetry run playwright install
```

Do działania wymagany plik `run.env` z danymi kluczem API TMDB (opcjonalnie) oraz preferencjami użytkownika w katalogu głównym projektu (patrz plik: `run.env.template`).

Możliwe także podanie tych danych jako zmienne środowiskowe.

## Uruchomienie

```shell
$ poetry run app repertoire # wyświetla repertuar dla domyślnego kina i daty
$ poetry run app repertoire bemowo 2024-12-06 # wyświetla repertuar dla kina Warszawa - Bemowo na podany dzień (o ile jest na stronie)
$ poetry run app venues list # wyświetla dostępne kina
$ poetry run app venues update # aktualizuje listę kin w lokalnej bazie danych
$ poetry run app venues search manufaktura # wyświetla kina zawierające w nazwie "manufaktura"
```

## Testy

Komendy do uruchamiania testów wybiórczo w pliku `Makefile`.

```shell
make tests
lub
poetry run pytest tests
```

| :warning: | Program nie jest aktywnie rozwijany, ale ewentualne PRy pod warunkiem pokrycia testami będą mergowane. |
| --------- | :----------------------------------------------------------------------------------------------------- |

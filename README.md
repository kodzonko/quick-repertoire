<img src=https://github.com/kodzonko/cinema-repertoire-analyzer/blob/master/docs/demo.gif/ width="80%">

# Cinema Repertoire Analyzer

![Coveralls](https://img.shields.io/coverallsCoverage/github/kodzonko/cinema-repertoire-analyzer) ![Python Version from PEP 621 TOML](https://img.shields.io/python/required-version-toml?tomlFilePath=https%3A%2F%2Fraw.githubusercontent.com%2Fkodzonko%2Fcinema-repertoire-analyzer%2Fmaster%2Fpyproject.toml) ![GitHub last commit](https://img.shields.io/github/last-commit/kodzonko/cinema-repertoire-analyzer)

Scraper repertuarow kin, przygotowany obecnie dla Cinema City i gotowy na rozszerzanie o kolejne sieci.
Wyniki sa pokazywane wraz z ocena i streszczeniem filmu z TMDB.

Date (w tym wartosci takie jak `dzis`, `jutro`) oraz domyslne lokale per siec mozna podac w parametrach lub ustawic w pliku `run.env`.

## Wymagania

- uv
- python 3.14
- (opcjonalnie) make

## Instalacja

```shell
$ uv sync
```

Przy pierwszym uruchomieniu Selenium moze pobrac przegladarke lub sterownik przez Selenium Manager.

Do dzialania wymagany jest plik `run.env` z kluczem API TMDB (opcjonalnie) oraz preferencjami uzytkownika w katalogu glownym projektu. Przyklad znajduje sie w `run.env.template`.

Mozliwe jest tez podanie tych danych jako zmienne srodowiskowe.

## Uruchomienie

```shell
$ uv run app repertoire --chain cinema-city
$ uv run app repertoire --chain cinema-city bemowo 2024-12-06
$ uv run app venues list --chain cinema-city
$ uv run app venues update --chain cinema-city
$ uv run app venues search --chain cinema-city manufaktura
```

## Testy

Komendy do uruchamiania testow wybiorczo znajduja sie w `Makefile`.

```shell
uv run pytest tests
```

| :warning: | Program nie jest aktywnie rozwijany, ale ewentualne PRy pod warunkiem pokrycia testami beda mergowane. |
| --------- | :------------------------------------------------------------------------------------------------------ |

[Post-mortem](https://github.com/kodzonko/cinema-repertoire-analyzer/blob/master/docs/post-mortem.md)

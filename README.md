# Cinema Repertoire Analyzer

![Demo](docs/demo.gif)

Scraper repertuarow kin, przygotowany obecnie dla Cinema City i gotowy na
rozszerzanie o kolejne sieci. Wyniki sa pokazywane wraz z ocena i
streszczeniem filmu z TMDB.

Date (w tym wartosci takie jak `dzis`, `jutro`) oraz domyslne lokale per siec
mozna podac w parametrach lub ustawic w pliku `run.env`.

## Wymagania

- uv
- python 3.14
- (opcjonalnie) make

## Instalacja

```shell
uv sync
```

Przy pierwszym uruchomieniu Selenium moze pobrac przegladarke lub sterownik
przez Selenium Manager.

Do dzialania wymagany jest plik `run.env` z kluczem API TMDB (opcjonalnie) oraz
preferencjami uzytkownika w katalogu glownym projektu. Przyklad znajduje sie w
`run.env.template`.

Mozliwe jest tez podanie tych danych jako zmienne srodowiskowe.

## Uruchomienie

```shell
uv run app repertoire --chain cinema-city
uv run app repertoire --chain cinema-city bemowo 2024-12-06
uv run app venues list --chain cinema-city
uv run app venues update --chain cinema-city
uv run app venues search --chain cinema-city manufaktura
```

## Testy

Komendy do uruchamiania testow wybiorczo znajduja sie w `Makefile`.

```shell
uv run pytest tests
```

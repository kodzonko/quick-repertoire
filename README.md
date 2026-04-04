# Quick repertoire

`quickrep` to proste narzędzie CLI do sprawdzania repertuaru kin.
Obsługuje `Cinema City`, `Helios` i `Multikino`, a opcjonalnie pokazuje też
oceny i opisy filmów z TMDB.

## Wymagania

- zainstalowany `Chrome` lub `Chromium`
- opcjonalnie klucz lub token `TMDB`, jeśli chcesz widzieć oceny i opisy filmów - darmowy: [https://www.themoviedb.org/settings/api](https://www.themoviedb.org/settings/api)

## Start

Przykłady niżej używają polecenia `quickrep` tak jakby było w PATH. Jeśli nie jest, trzeba podać ścieżkę do pliku np. `/Users/kodzonko/Downlaods/quickrep`, `C:\Users\kodzonko\Downlaods\quickrep`.

Przy pierwszym uruchomieniu:

```shell
quickrep configure
```

Kreator zapisze obok programu `config.ini` i `db.sqlite`, a potem możesz używać
krótkich poleceń bez podawania wszystkiego za każdym razem.

## Komendy i przykłady

```shell
# Wypisze wspierane sieci kin
quickrep chains

# Wypisze repertuar dla domyślnej sieci, kina i daty
quickrep repertoire
quickrep repertoire bemowo
quickrep repertoire bemowo jutro
# Wypisze repertuar dla domyślnej sieci, konkretnego kina i daty
quickrep repertoire manufaktura 2026-04-05
quickrep repertoire --chain helios
quickrep repertoire --chain multikino poznan

quickrep venues list
quickrep venues list --chain cinema-city
quickrep venues search manufaktura
quickrep venues search mokotow --chain helios
# Wymusi odświeżenie bazy kin
quickrep venues update
quickrep venues update --chain helios
```

- `repertoire` bez argumentów używa domyślnej sieci, kina i daty z konfiguracji
- data może być podana jako `dziś`, `jutro` albo `YYYY-MM-DD` / `DD-MM-YYYY` / `DD.MM.YYYY`
- `--chain` przyjmuje: `cinema-city`, `helios`, `multikino`
- `venues search` szuka po fragmencie nazwy kina
- `venues update` odświeża lokalną bazę kin
- `TMDB` jest opcjonalne; bez niego repertuar działa dalej, ale bez ocen i opisów

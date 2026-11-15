# Quick repertoire

`quickrep` to proste narzędzie CLI do sprawdzania repertuaru kin.
Obsługuje `Cinema City`, `Helios` i `Multikino`, a opcjonalnie pokazuje też
oceny i opisy filmów z TMDB.

## Tool setup

Install [mise](https://mise.jdx.dev/getting-started.html), then run from the repository root:

```sh
mise trust
mise install
```

Tool versions are declared in `mise.toml`. Activate mise in your shell, or prefix
project commands with `mise exec --` (for example, `mise exec -- cargo --version`).

## Coverage

The Linux job in `Rust Test Suite` generates an LCOV report and uploads it to
[Codecov](https://app.codecov.io/gh/kodzonko/quick-repertoire) on pushes to `main`
and pull requests. Enable this repository in Codecov before the first CI run.
Uploads use GitHub OIDC, so no `CODECOV_TOKEN` secret is needed. Upload errors
fail the job.

To generate the same report locally:

```sh
cargo llvm-cov --locked --all-targets --all-features
cargo llvm-cov report --lcov --output-path target/lcov.info
```

## Wymagania

- zainstalowany `Chrome` lub `Chromium`
- opcjonalnie klucz lub token `TMDB`, jeśli chcesz widzieć oceny i opisy filmów - darmowy: [https://www.themoviedb.org/settings/api](https://www.themoviedb.org/settings/api)

## Start

Przykłady niżej używają polecenia `quickrep` tak jakby było w PATH. Jeśli nie jest, trzeba podać ścieżkę do pliku np. `/Users/kodzonko/Downlaods/quickrep`, `C:\Users\kodzonko\Downlaods\quickrep`.

Przy pierwszym uruchomieniu:

```shell
quickrep configure
```

Kreator zapisze preferencje i listę kin w jednym pliku `db.sqlite` obok programu.
Odświeżanie listy kin zachowuje preferencje. Usunięcie bazy usuwa również ustawienia;
przy następnym uruchomieniu trzeba ponownie przejść konfigurator.

Ustawienia ze starego `config.json` lub `config.ini` zostaną automatycznie przeniesione
do bazy przy wczytaniu konfiguracji, jeśli baza nie zawiera jeszcze preferencji.
JSON ma pierwszeństwo przed INI. Stare pliki pozostają bez zmian i po migracji
można je usunąć. Aplikacja zapisuje nowe ustawienia wyłącznie w SQLite.
Token TMDB jest przechowywany bez szyfrowania; na systemach Unix baza ma uprawnienia
`0600` (odczyt i zapis tylko dla właściciela).

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

## Testy z prawdziwymi serwisami

Testy w `tests/live_e2e.rs` pobierają aktualne repertuary Cinema City, Helios i
Multikino oraz sprawdzają dopasowania TMDB. Uruchamiają się osobno, aby zwykły
`cargo test` działał bez sieci i klucza API:

```shell
TMDB_ACCESS_TOKEN=... cargo test --test live_e2e -- --ignored --test-threads=1
```

Harmonogram GitHub Actions uruchamia je co poniedziałek. W repozytorium trzeba
ustawić sekret `TMDB_ACCESS_TOKEN` (klucz API lub API Read Access Token).
Testy sprawdzają obecność seansów i wypełnienie większości podstawowych pól,
bez przywiązywania się do konkretnych tytułów czy godzin.

## Testy wydajności

```shell
cargo test --release --test live_performance -- --ignored --nocapture --test-threads=1
# Tylko Cinema City:
cargo test --release --test live_performance cinema_city -- --ignored --nocapture
```

Testy obejmują po dwa kina: Cinema City Manufaktura i Wroclavia, Helios w Łodzi
i Wrocławiu oraz Multikino Złote Tarasy i Poznań. Każdy pomiar uruchamia nowy
proces CLI z tymczasową konfiguracją i gotową bazą kin. Dwa kolejne pomiary
pokazują koszt startu procesu; nie czyszczą cache systemu ani DNS. Pobieranie
listy kin jest mierzone osobno. Wyniki zawierają czas całego procesu oraz etapy:
wyszukanie kina, pobranie repertuaru i TMDB. Domyślnie TMDB jest wyłączone;
ustawienie `TMDB_ACCESS_TOKEN` dodaje pomiary z ocenami i opisami.

Testy wymagają sieci oraz Chrome/Chromium. Raportują czasy bez stałego limitu,
ponieważ szybkość zewnętrznych serwisów jest zmienna. Zwykły `cargo test`
sprawdza współbieżność żądań TMDB na lokalnym serwerze. Osobny test przeglądarki
sprawdza, czy JavaScript działa bez pobierania obrazów:

```shell
cargo test --test browser_test -- --ignored
RUST_LOG=quick_repertoire::app=debug quickrep repertoire
```

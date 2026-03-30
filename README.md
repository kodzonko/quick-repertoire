# Quick repertoire

Terminalowy scraper repertuarów kin napisany w Rust. Aktualnie obsługuje
Cinema City, zapisuje konfigurację w `config.ini`, cache'uje lokale w SQLite i
wzbogaca repertuar o oceny oraz opisy z TMDB.

## Wymagania

- Rust 1.94+
- cargo
- zainstalowany Chrome lub Chromium

## Instalacja

```shell
cargo build
```

## Konfiguracja

Aplikacja zapisuje konfigurację w `config.ini` w katalogu głównym repo.
Przy pierwszym uruchomieniu uruchomi się interaktywny kreator:

- pobierze listy lokali dla wszystkich obsługiwanych sieci z widocznym postępem
- pozwoli wybrać domyślną sieć, lokal, datę repertuaru, poziom logowania i plik bazy
- zapyta o opcjonalny token TMDB

Aby uruchomić kreator ponownie:

```shell
cargo run --bin app -- configure
```

## Uruchomienie

```shell
cargo run --bin app -- repertoire
cargo run --bin app -- repertoire bemowo 2024-12-06
cargo run --bin app -- repertoire --chain cinema-city
cargo run --bin app -- venues list
cargo run --bin app -- venues update
cargo run --bin app -- venues search manufaktura
```

Polecenia nadal przyjmują jawne `--chain`, ale gdy go pominiesz, aplikacja użyje
domyślnej sieci z `config.ini`.

## Testy i jakość

Codzienny przepływ dla kontroli jakości:

```shell
cargo fmt --all
cargo fmtcheck
cargo lint
cargo test
```

`cargo fmt --all` formatuje kod według ustawień z `rustfmt.toml`.
`cargo fmtcheck` uruchamia `cargo fmt --all --check`.
`cargo lint` uruchamia `cargo clippy --all-targets --all-features -- -D warnings`.

Jeśli wolisz uruchamiać pełne komendy bez aliasów z `.cargo/config.toml`:

```shell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

# Quick repertoire

Terminalowy scraper repertuarów kin napisany w Rust. Aktualnie obsługuje
Cinema City, Helios i Multikino, zapisuje konfigurację w `config.ini` obok binarki,
cache'uje lokale w SQLite i wzbogaca repertuar o oceny oraz opisy z TMDB.

## Wymagania

- Rust 1.94+
- cargo
- zainstalowany Chrome lub Chromium

## Instalacja

```shell
cargo build --release
```

Po zbudowaniu uruchamiaj gotową binarkę, np. `./target/release/quickrep`.

## Konfiguracja

Aplikacja zapisuje `config.ini` oraz `db.sqlite` w katalogu binarki.
Przy lokalnym buildzie oznacza to zwykle `target/release`.
Przy pierwszym uruchomieniu uruchomi się interaktywny kreator:

- pobierze listy lokali dla wszystkich obsługiwanych sieci z widocznym postępem
- pozwoli wybrać domyślną sieć, lokal i datę repertuaru
- zapyta o opcjonalne dane uwierzytelniające TMDB
- przed zapisem sprawdzi, czy katalog binarki pozwala na utworzenie lub modyfikację
  plików konfiguracyjnych

Do pola TMDB możesz wkleić jedno z dwóch pól widocznych w ustawieniach konta:

- `Przeczytaj kod odczytu API` (`API Read Access Token`) - zalecane
- `Klucz API` (`API Key`) - też działa

Obie opcje dają w tej aplikacji ten sam efekt. Zalecany jest `API Read Access Token`,
bo TMDB traktuje go jako domyślny sposób autoryzacji i można go używać zarówno z API v3,
jak i v4. Jeśli wkleisz 32-znakowy `Klucz API`, aplikacja użyje go w trybie zgodnym z v3.

Źródło: oficjalna dokumentacja TMDB o autoryzacji aplikacji:
[Authentication: Application](https://developer.themoviedb.org/docs/authentication-application).

Poziom logowania nie jest zapisywany w `config.ini`. Buildy developerskie oraz testy
domyślnie używają `DEBUG`, a buildy produkcyjne `INFO`.

Aby uruchomić kreator ponownie:

```shell
./target/release/quickrep configure
```

## Uruchomienie

```shell
./target/release/quickrep repertoire
./target/release/quickrep repertoire bemowo 2024-12-06
./target/release/quickrep repertoire --chain cinema-city
./target/release/quickrep repertoire --chain helios
./target/release/quickrep repertoire --chain multikino
./target/release/quickrep venues list
./target/release/quickrep venues update
./target/release/quickrep venues search manufaktura
```

Polecenia nadal przyjmują jawne `--chain`. Gdy pominiesz go w `repertoire`,
`venues list` albo `venues search`, aplikacja użyje domyślnej sieci z `config.ini`.
`venues update` bez `--chain` odświeża równolegle wszystkie obsługiwane sieci,
pokazując spinner dla każdej z nich oraz zbiorczy pasek postępu.

## Testy i jakość

```shell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Aby uruchomić raport pokrycia lokalnie, doinstaluj `cargo-llvm-cov` i użyj:

```shell
cargo install cargo-llvm-cov
cargo coverage
```

Na Windows domyślny alias używa zwykłego raportu, bo `llvm-cov --branch` na nightly potrafi się wysypać.

Pokrycie branchy jest liczone w CI na Linuxie. Jeśli chcesz uruchomić je lokalnie, użyj nightly na Linuxie/WSL i wywołaj `cargo llvm-cov` bez aliasu:

```shell
rustup toolchain install nightly
cargo +nightly llvm-cov --branch --all-targets --all-features
```

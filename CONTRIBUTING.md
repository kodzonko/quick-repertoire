# Współtworzenie

Dzięki za chęć współtworzenia `quick-repertoire`.

## Zanim zaczniesz

- Sprawdź istniejące issues i pull requesty, zanim otworzysz nowe zgłoszenie.
- Przy regresjach scrapera podaj sieć kin, konkretne kino, datę oraz komendę,
  której użyłeś.
- Przy propozycjach nowych funkcji najpierw opisz problem użytkownika, a dopiero
  potem oczekiwane zachowanie CLI.

## Pull requesty

- Otwieraj pull requesty względem `master`.
- Utrzymuj mały zakres zmian. Jeśli to możliwe, oddzielaj refaktoryzację od
  zmian zachowania.
- Dodawaj albo aktualizuj testy dla poprawek błędów i zmian zachowania.
- Używaj najnowszych stabilnych zależności, chyba że jest wyraźny powód, by
  tego nie robić.

Przed otwarciem lub aktualizacją pull requesta uruchom:

```shell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Commity i wydania

- Twórz jasne, celowe commity.
- Tagi wydań używają numerycznego formatu, na przykład `1.2.3`.
- Jeśli zmiana powinna trafić do notatek wydania, dodaj odpowiednie etykiety
  pasujące do kategorii changeloga.

## Zgłaszanie problemów bezpieczeństwa

Nie zgłaszaj problemów bezpieczeństwa publicznie w issues. Zastosuj się do
instrukcji z [`SECURITY.md`](SECURITY.md).

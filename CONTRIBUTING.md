# Contributing

Thanks for contributing to `quick-repertoire`.

## Before you start

- Check existing issues and pull requests before opening a new one.
- For scraper regressions, include the affected cinema chain, venue, date, and
  the command you ran.
- For feature proposals, describe the user problem first, then the CLI behavior
  you want to add or change.

## Pull requests

- Open pull requests against `master`.
- Keep changes scoped. Separate refactors from behavior changes when possible.
- Add or update tests for behavior changes and bug fixes.
- Use the latest stable dependencies unless there is a clear reason not to.

Before opening or updating a pull request, run:

```shell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Commit and release conventions

- Use clear, intentional commits.
- Release tags use numeric versions such as `1.2.3`.
- If a change affects release notes, add labels that match the release
  categories where possible.

## Reporting security issues

Please do not file public issues for security problems. Follow
[`SECURITY.md`](SECURITY.md) instead.

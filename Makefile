.PHONY: test e2e build lint fmt changelog release-notes

test:
	cargo test

e2e:
	dotenvx run -- cargo test --test live_e2e -- --ignored --test-threads=1

build:
	cargo build --release

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

# Prepend the section for a new release: make changelog VERSION=1.2.0
changelog:
	@test -n "$(VERSION)" || { echo "usage: make changelog VERSION=x.y.z" >&2; exit 1; }
	git cliff --unreleased --tag v$(VERSION) --prepend CHANGELOG.md

# Print GitHub release notes for the latest tag (the changelog section without its heading)
release-notes:
	@git cliff --latest --strip all | sed '1{/^## /d;}'

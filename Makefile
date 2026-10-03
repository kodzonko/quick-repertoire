.PHONY: test e2e build lint fmt

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

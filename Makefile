.PHONY: fmt lint run

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	cargo run -p pandemonium

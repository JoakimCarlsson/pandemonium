.PHONY: fmt lint run release

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	cargo run -p pandemonium

release:
	scripts/release.sh $(VERSION)

.PHONY: fmt lint run install release

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	cargo run -p pandemonium

install:
	scripts/install-preview.sh

release:
	scripts/release.sh $(VERSION)

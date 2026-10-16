.PHONY: fmt lint run install install-server release languages

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	cargo run -p pandemonium

install:
	scripts/install-preview.sh

install-server:
	scripts/install-preview.sh server

release:
	scripts/release.sh $(VERSION)

languages:
	cargo run -p pm-server --bin pandemonium-server -- --index-languages languages

.PHONY: fmt lint run server install release

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run: server
	cargo run -p pandemonium

server:
	sh scripts/build-server.sh debug

install:
	scripts/install-preview.sh

release:
	scripts/release.sh $(VERSION)

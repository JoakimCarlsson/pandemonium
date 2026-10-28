.PHONY: fmt lint run install install-server uninstall release languages extension-sample

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	XDG_DATA_HOME="$${XDG_DATA_HOME:-$$HOME/.local/share/pandemonium-dev}" cargo run -p pandemonium

install:
	scripts/install-preview.sh

install-server:
	scripts/install-preview.sh server

uninstall:
	scripts/uninstall-preview.sh

release:
	scripts/release.sh $(VERSION)

languages:
	cargo run -p pm-server --bin pandemonium-server -- --index-languages languages

extension-sample:
	cargo build --release --target wasm32-unknown-unknown --manifest-path extensions/hello-guest/Cargo.toml --target-dir build/hello-guest
	rm -rf build/extensions/hello
	mkdir -p build/extensions
	cp -r extensions/hello build/extensions/hello
	cp build/hello-guest/wasm32-unknown-unknown/release/hello_guest.wasm build/extensions/hello/extension.wasm
	cargo build --release --target wasm32-unknown-unknown --manifest-path extensions/sample-server-guest/Cargo.toml --target-dir build/sample-server-guest
	rm -rf build/extensions/sample-server
	cp -r extensions/sample-server build/extensions/sample-server
	cp build/sample-server-guest/wasm32-unknown-unknown/release/sample_server_guest.wasm build/extensions/sample-server/extension.wasm

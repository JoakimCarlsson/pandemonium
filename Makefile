.PHONY: fmt lint run browser-helper install install-server uninstall local preview release languages extension-sample

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run: browser-helper
	XDG_DATA_HOME="$${XDG_DATA_HOME:-$$HOME/.local/share/pandemonium-dev}" PANDEMONIUM_BROWSER_HELPER="$(CURDIR)/helpers/electron-browser/pandemonium-browser-electron" cargo run -p pandemonium

browser-helper: helpers/electron-browser/node_modules/electron/path.txt

helpers/electron-browser/node_modules/electron/path.txt: helpers/electron-browser/package-lock.json
	cd helpers/electron-browser && npm ci --no-audit --no-fund && node node_modules/electron/install.js

TARGET := $(or $(filter local preview,$(MAKECMDGOALS)),local)

local preview:
	@:

install:
	scripts/install-checkout.sh $(TARGET)

install-server:
	scripts/install-checkout.sh server

uninstall:
	scripts/uninstall-checkout.sh $(TARGET)

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

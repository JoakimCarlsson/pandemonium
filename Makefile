.PHONY: fmt lint run install install-server release languages language-modules extension-sample

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

release:
	scripts/release.sh $(VERSION)

languages:
	cargo run -p pm-server --bin pandemonium-server -- --index-languages languages

# Builds the modules some language folders carry (`module.wasm`, from `module/`) and puts them in place.
language-modules:
	for module in mermaid math; do \
		cargo build --release --target wasm32-unknown-unknown --manifest-path languages/$$module/module/Cargo.toml --target-dir build/module-$$module && \
		cp build/module-$$module/wasm32-unknown-unknown/release/$${module}_module.wasm languages/$$module/module.wasm; \
	done

extension-sample:
	cargo build --release --target wasm32-unknown-unknown --manifest-path extensions/hello-guest/Cargo.toml --target-dir build/hello-guest
	rm -rf build/extensions/hello
	mkdir -p build/extensions
	cp -r extensions/hello build/extensions/hello
	cp build/hello-guest/wasm32-unknown-unknown/release/hello_guest.wasm build/extensions/hello/extension.wasm

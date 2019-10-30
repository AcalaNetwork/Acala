run: githooks
	SKIP_WASM_BUILD= cargo run -- --dev --execution native

toolchain:
	./scripts/init.sh

build-wasm: githooks
	WASM_BUILD_TYPE=release cargo build

check: githooks
	SKIP_WASM_BUILD= cargo check

check-dummy:
	BUILD_DUMMY_WASM_BINARY= cargo check

build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/acala
	target/debug/acala purge-chain --dev -y

restart: purge run

target/debug/acala: build

GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

githooks: .git/hooks $(GITHOOKS_DEST)

init: toolchain build-wasm
	git submodule update --init --recursive

update:
	cd orml && git pull

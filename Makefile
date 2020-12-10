run: githooks
	SKIP_WASM_BUILD= cargo run --manifest-path bin/acala-dev/Cargo.toml -- --dev -lruntime=debug --instant-sealing

run-eth: githooks
	cargo run --manifest-path bin/acala-dev/Cargo.toml --features with-ethereum-compatibility -- --dev -lruntime=debug -levm=debug --instant-sealing

toolchain:
	./scripts/init.sh

build-full: githooks
	cargo build

check: githooks
	SKIP_WASM_BUILD= cargo check

check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

check-runtime: check-dev-runtime check-all-runtime

check-dev-runtime:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala-dev/Cargo.toml --tests --all --features with-all-runtime

check-all-runtime:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala/Cargo.toml --tests --all --features with-all-runtime

check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

test: githooks
	SKIP_WASM_BUILD= cargo test --all

test-runtime: test-dev-runtime test-all-runtime

test-dev-runtime:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala/Cargo.toml --all --features with-all-runtime

test-all-runtime:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala/Cargo.toml --all --features with-all-runtime

build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/acala-dev
	target/debug/acala-dev purge-chain --dev -y

restart: purge run

target/debug/acala-dev: build

GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

githooks: .git/hooks $(GITHOOKS_DEST)

init: toolchain submodule build-full

submodule:
	git submodule update --init --recursive

update-orml:
	cd orml && git checkout master && git pull
	git add orml

update: update-orml
	cargo update
	make check

build-wasm-mandala:
	./scripts/build-only-wasm.sh mandala-runtime

build-wasm-karura:
	./scripts/build-only-wasm.sh karura-runtime

build-wasm-acala:
	./scripts/build-only-wasm.sh acala-runtime

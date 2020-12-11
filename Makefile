.PHONY: run
run: githooks
	SKIP_WASM_BUILD= cargo run --manifest-path bin/acala-dev/Cargo.toml -- --dev -lruntime=debug --instant-sealing

.PHONY: run-eth
run-eth: githooks
	cargo run --manifest-path bin/acala-dev/Cargo.toml --features with-ethereum-compatibility -- --dev -lruntime=debug -levm=debug --instant-sealing

.PHONY: toolchain
toolchain:
	./scripts/init.sh

.PHONY: build-full
build-full: githooks
	cargo build

build-all: build-dev build-acala

build-dev:
	cargo build --manifest-path bin/acala-dev/Cargo.toml --locked

build-acala:
	cargo build --manifest-path bin/acala/Cargo.toml --locked --features with-all-runtime

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

check-all: check-dev check-acala

check-dev:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala-dev/Cargo.toml --tests --all

check-acala:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala/Cargo.toml --tests --all --features with-all-runtime

.PHONY: check-debug
check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= cargo test --all

test-all: test-dev test-acala

test-dev:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala/Cargo.toml --all

test-acala:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala/Cargo.toml --all --features with-all-runtime

.PHONY: build
build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/acala-dev
	target/debug/acala-dev purge-chain --dev -y

.PHONY: restart
restart: purge run

target/debug/acala-dev: build
	SKIP_WASM_BUILD= cargo build

GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

.PHONY: githooks
githooks: .git/hooks $(GITHOOKS_DEST)

.PHONY: init
init: toolchain submodule build-full

.PHONY: submodule
submodule:
	git submodule update --init --recursive

.PHONY: update-orml
update-orml:
	cd orml && git checkout master && git pull
	git add orml

.PHONY: update
update: update-orml
	cargo update
	make check

.PHONY: build-wasm-mandala
build-wasm-mandala:
	./scripts/build-only-wasm.sh mandala-runtime

.PHONY: build-wasm-karura
build-wasm-karura:
	./scripts/build-only-wasm.sh karura-runtime

.PHONY: build-wasm-acala
build-wasm-acala:
	./scripts/build-only-wasm.sh acala-runtime

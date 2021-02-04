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

.PHONY: build-all
build-all: build-dev build-acala

.PHONY: build-dev
build-dev:
	cargo build --manifest-path bin/acala-dev/Cargo.toml --locked

.PHONY: build-acala
build-acala:
	cargo build --manifest-path bin/acala/Cargo.toml --locked --features with-all-runtime

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

.PHONY: check-all
check-all: check-dev check-acala

.PHONY: check-dev
check-dev:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala-dev/Cargo.toml --tests --all

.PHONY: check-acala
check-acala:
	SKIP_WASM_BUILD= cargo check --manifest-path bin/acala/Cargo.toml --tests --all --features with-all-runtime

.PHONY: check-debug
check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= cargo test --all

.PHONY: test-eth
test-eth: githooks
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala-dev/Cargo.toml test_evm_module --features with-ethereum-compatibility -p mandala-runtime

.PHONY: test-all
test-all: test-dev test-acala

.PHONY: test-dev
test-dev:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala-dev/Cargo.toml --all

.PHONY: test-acala
test-acala:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala/Cargo.toml --all --features with-all-runtime

.PHONY: test-benchmarking
test-benchmarking:
	SKIP_WASM_BUILD= cargo test --manifest-path bin/acala-dev/Cargo.toml --features runtime-benchmarks -p mandala-runtime benchmarking

.PHONY: build
build: githooks
	SKIP_WASM_BUILD= cargo build

.PHONY: purge
purge: target/debug/acala-dev
	target/debug/acala-dev purge-chain --dev -y

.PHONY: restart
restart: purge run

target/debug/acala-dev:
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

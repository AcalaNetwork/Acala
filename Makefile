.PHONY: run
run: githooks
	cargo run -- --dev -lruntime=debug --instant-sealing

.PHONY: run-eth
run-eth: githooks
	cargo run --features with-ethereum-compatibility -- --dev -lruntime=debug -levm=debug --instant-sealing

.PHONY: toolchain
toolchain:
	./scripts/init.sh

.PHONY: build
build: githooks
	SKIP_WASM_BUILD= cargo build

.PHONY: build-full
build-full: githooks
	cargo build

.PHONY: build-all
build-all:
	cargo build --locked --features with-all-runtime

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

.PHONY: check-all
check-all: check-runtimes check-benchmarks

.PHONY: check-runtimes
check-runtimes:
	SKIP_WASM_BUILD= cargo check --tests --all --features with-all-runtime

.PHONY: check-benchmarks
check-benchmarks:
	SKIP_WASM_BUILD= cargo check --features runtime-benchmarks --no-default-features --target=wasm32-unknown-unknown -p mandala-runtime
	SKIP_WASM_BUILD= cargo check --features runtime-benchmarks --no-default-features --target=wasm32-unknown-unknown -p karura-runtime

.PHONY: check-debug
check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= cargo test --all

.PHONY: test-eth
test-eth: githooks
	SKIP_WASM_BUILD= cargo test test_evm_module --features with-ethereum-compatibility -p mandala-runtime

.PHONY: test-all
test-all: test-runtimes test-benchmarking

.PHONY: test-runtimes
test-runtimes:
	SKIP_WASM_BUILD= cargo test --all --features with-all-runtime

.PHONY: test-benchmarking
test-benchmarking:
	SKIP_WASM_BUILD= cargo test --features runtime-benchmarks --features with-all-runtime --features --all benchmarking

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
update: update-orml cargo-update check-all

.PHONY: cargo-update
cargo-update:
	cargo update

.PHONY: build-wasm-mandala
build-wasm-mandala:
	./scripts/build-only-wasm.sh mandala-runtime

.PHONY: generate-tokens
generate-tokens:
	./scripts/generate-tokens-and-predeploy-contracts.sh

.PHONY: run
run:
	cargo run --features with-mandala-runtime -- --dev -lruntime=debug --instant-sealing

.PHONY: run-eth
run-eth:
	cargo run --features with-mandala-runtime --features with-ethereum-compatibility -- --dev -lruntime=debug -levm=debug --instant-sealing

.PHONY: run-karura
run-karura:
	cargo run --features with-karura-runtime -- --chain=karura

.PHONY: run-acala
run-acala:
	cargo run --features with-acala-runtime -- --chain=acala

.PHONY: toolchain
toolchain:
	./scripts/init.sh

.PHONY: build
build: githooks
	SKIP_WASM_BUILD= cargo build --features with-mandala-runtime

.PHONY: build-full
build-full: githooks
	cargo build --features with-mandala-runtime

.PHONY: build-all
build-all:
	cargo build --locked --features with-all-runtime

.PHONY: build-benches
build-benches:
	cargo bench --locked --no-run --features bench --package module-evm
	cargo bench --locked --no-run --features bench --package runtime-common

.PHONY: build-release
build-release:
	cargo build --locked --features with-all-runtime --profile production --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-mandala-release
build-mandala-release:
	cargo build --locked --features with-mandala-runtime --profile production --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-karura-release
build-karura-release:
	cargo build --locked --features with-karura-runtime --profile production --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-acala-release
build-acala-release:
	cargo build --locked --features with-acala-runtime --profile production --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-mandala-internal-release
build-mandala-internal-release:
	cargo build --locked --features with-mandala-runtime --release --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-karura-internal-release
build-karura-internal-release:
	cargo build --locked --features with-karura-runtime --release --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: build-acala-internal-release
build-acala-internal-release:
	cargo build --locked --features with-acala-runtime --release --workspace --exclude runtime-integration-tests --exclude e2e-tests --exclude test-service

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check --features with-mandala-runtime

.PHONY: check
check-karura: githooks
	SKIP_WASM_BUILD= cargo check --features with-karura-runtime

.PHONY: check
check-acala: githooks
	SKIP_WASM_BUILD= cargo check --features with-acala-runtime

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --features with-all-runtime --tests --all

.PHONY: check-all
check-all: check-runtimes check-benchmarks check-integration-tests

.PHONY: check-runtimes
check-runtimes:
	SKIP_WASM_BUILD= cargo check --features with-all-runtime --tests --all

.PHONY: check-benchmarks
check-benchmarks:
	SKIP_WASM_BUILD= cargo check --features bench --package module-evm
	SKIP_WASM_BUILD= cargo check --features runtime-benchmarks --no-default-features --target=wasm32-unknown-unknown -p mandala-runtime
	SKIP_WASM_BUILD= cargo check --features runtime-benchmarks --no-default-features --target=wasm32-unknown-unknown -p karura-runtime
	SKIP_WASM_BUILD= cargo check --features runtime-benchmarks --no-default-features --target=wasm32-unknown-unknown -p acala-runtime

.PHONY: check-integration-tests
check-integration-tests:
	SKIP_WASM_BUILD= cargo check -p runtime-integration-tests --features=with-mandala-runtime
	SKIP_WASM_BUILD= cargo check -p runtime-integration-tests --features=with-karura-runtime
	SKIP_WASM_BUILD= cargo check -p runtime-integration-tests --features=with-acala-runtime

.PHONY: check-debug
check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check --features with-mandala-runtime

.PHONY: check-try-runtime
check-try-runtime:
	SKIP_WASM_BUILD= cargo check --features try-runtime --features with-all-runtime

.PHONY: try-runtime-karura
try-runtime-karura:
	SKIP_WASM_BUILD= cargo run --features with-karura-runtime --features try-runtime -- try-runtime --chain=karura-dev --wasm-execution=compiled on-runtime-upgrade live --uri wss://karura.api.onfinality.io:443/public-ws -s /tmp/snapshot.bin

.PHONY: try-runtime-mandala
try-runtime-mandala:
	SKIP_WASM_BUILD= cargo run --features with-mandala-runtime --features try-runtime -- try-runtime --chain=dev --wasm-execution=compiled on-runtime-upgrade live --uri wss://mandala.polkawallet.io:443 -s /tmp/snapshot.bin

.PHONY: try-runtime-acala
try-runtime-acala:
	SKIP_WASM_BUILD= cargo run --features with-acala-runtime --features try-runtime -- try-runtime --chain=acala-dev --wasm-execution=compiled on-runtime-upgrade live --uri wss://acala-polkadot.api.onfinality.io:443/public-ws -s /tmp/snapshot.bin

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= cargo test --features with-mandala-runtime --all

.PHONY: test-eth
test-eth: githooks test-evm
	SKIP_WASM_BUILD= cargo test -p runtime-common --features with-ethereum-compatibility schedule_call_precompile_should_work
	SKIP_WASM_BUILD= cargo test -p runtime-integration-tests --features with-mandala-runtime --features with-ethereum-compatibility should_not_kill_contract_on_transfer_all
	SKIP_WASM_BUILD= cargo test -p runtime-integration-tests --features with-mandala-runtime --features with-ethereum-compatibility schedule_call_precompile_should_handle_invalid_input

.PHONY: test-evm
test-evm: githooks
	SKIP_WASM_BUILD= cargo test --manifest-path evm-tests/jsontests/Cargo.toml

.PHONY: test-runtimes
test-runtimes:
	SKIP_WASM_BUILD= cargo test --all --features with-all-runtime
	SKIP_WASM_BUILD= cargo test -p runtime-integration-tests --features=with-mandala-runtime
	SKIP_WASM_BUILD= cargo test -p runtime-integration-tests --features=with-karura-runtime
	SKIP_WASM_BUILD= cargo test -p runtime-integration-tests --features=with-acala-runtime

.PHONY: test-e2e
test-e2e:
	cargo test --release --package test-service -- --include-ignored --skip test_full_node_catching_up --skip simple_balances_test

.PHONY: test-ts
test-ts: build-mandala-internal-release
	cd ts-tests && yarn && yarn run build && ACALA_BUILD=release yarn run test

.PHONY: test-benchmarking
test-benchmarking:
	cargo test --features bench --package module-evm --package runtime-common
	cargo test --features runtime-benchmarks --features with-all-runtime --features --all benchmarking

.PHONY: test-all
test-all: test-runtimes test-eth test-benchmarking

.PHONY: purge
purge: target/debug/acala
	target/debug/acala purge-chain --dev -y

.PHONY: restart
restart: purge run

target/debug/acala:
	SKIP_WASM_BUILD= cargo build --features with-mandala-runtime

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
	./scripts/build-only-wasm.sh --profile production -p mandala-runtime --features=on-chain-release-build --workspace --exclude runtime-integration-tests --exclude e2e-tests

.PHONY: build-wasm-karura
build-wasm-karura:
	./scripts/build-only-wasm.sh --profile production -p karura-runtime --features=on-chain-release-build --workspace --exclude runtime-integration-tests --exclude e2e-tests

.PHONY: build-wasm-acala
build-wasm-acala:
	./scripts/build-only-wasm.sh --profile production -p acala-runtime --features=on-chain-release-build --workspace --exclude runtime-integration-tests --exclude e2e-tests

.PHONY: srtool-build-wasm-mandala
srtool-build-wasm-mandala:
	PACKAGE=mandala-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: srtool-build-wasm-karura
srtool-build-wasm-karura:
	PACKAGE=karura-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: srtool-build-wasm-acala
srtool-build-wasm-acala:
	PACKAGE=acala-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: generate-tokens
generate-tokens:
	./scripts/generate-tokens-and-predeploy-contracts.sh

.PHONY: benchmark-mandala
benchmark-mandala:
	 cargo run --profile production --features=runtime-benchmarks --features=with-mandala-runtime -- benchmark --chain=mandala-latest --steps=50 --repeat=20 '--pallet=*' '--extrinsic=*' --execution=wasm --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/mandala/src/weights/

.PHONY: benchmark-karura
benchmark-karura:
	 cargo run --profile production --features=runtime-benchmarks --features=with-karura-runtime -- benchmark --chain=karura-dev --steps=50 --repeat=20 '--pallet=*' '--extrinsic=*' --execution=wasm --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/karura/src/weights/

.PHONY: benchmark-acala
benchmark-acala:
	 cargo run --profile production --features=runtime-benchmarks --features=with-acala-runtime -- benchmark --chain=acala-dev --steps=50 --repeat=20 '--pallet=*' '--extrinsic=*' --execution=wasm --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/acala/src/weights/

.PHONY: clippy-fix
clippy-fix:
	CARGO_INCREMENTAL=0 ./orml/scripts/run-clippy.sh --fix -Z unstable-options --broken-code --allow-dirty

.PHONY: bench-evm
bench-evm:
	cargo bench -p runtime-common --features bench | cargo run --bin weight-gen -- --template ./templates/precompile-weight-template.hbs --output runtime/common/src/precompile/weights.rs
	cargo bench -p module-evm --features bench | evm-bench/analyze_benches.js runtime/common/src/gas_to_weight_ratio.rs

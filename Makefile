# use `cargo nextest run` if cargo-nextest is installed
cargo_test = $(shell which cargo-nextest >/dev/null && echo "cargo nextest run" || echo "cargo test")
bunx_or_npx = $(shell which bunx >/dev/null && echo "bunx" || echo "npx")

.PHONY: run
run: chainspec-dev
	bunx @acala-network/chopsticks --chain-spec chainspecs/dev.json

.PHONY: run-acala-dev
run-acala-dev: chainspec-acala-dev
	bunx @acala-network/chopsticks --chain-spec chainspecs/acala-dev.json

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
	cargo build --locked

.PHONY: build-benches
build-benches:
	cargo bench --locked --no-run --features wasm-bench --package module-evm
	cargo bench --locked --no-run --features wasm-bench --package runtime-common

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

.PHONY: check-all
check-all: check-runtimes check-benchmarks check-tests check-integration-tests check-try-runtime

.PHONY: check-runtimes
check-runtimes:
	SKIP_WASM_BUILD= cargo check -p mandala-runtime --features "runtime-benchmarks try-runtime with-ethereum-compatibility on-chain-release-build" --tests
	SKIP_WASM_BUILD= cargo check -p karura-runtime --features "runtime-benchmarks try-runtime on-chain-release-build" --tests
	SKIP_WASM_BUILD= cargo check -p acala-runtime --features "runtime-benchmarks try-runtime on-chain-release-build" --tests

.PHONY: check-benchmarks
check-benchmarks:
	SKIP_WASM_BUILD= cargo check --features wasm-bench --package module-evm
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
	SKIP_WASM_BUILD= cargo check --features try-runtime

.PHONY: try-runtime-karura
try-runtime-karura:
	cargo build --release --locked --features with-karura-runtime --features try-runtime --bin acala
	./target/release/acala try-runtime --runtime ./target/release/wbuild/karura-runtime/karura_runtime.compact.compressed.wasm --chain=karura-dev on-runtime-upgrade live --uri wss://karura.api.onfinality.io:443/public-ws

.PHONY: try-runtime-mandala
try-runtime-mandala:
	cargo build --release --locked --features with-mandala-runtime --features try-runtime --bin acala
	./target/release/acala try-runtime --runtime ./target/release/wbuild/mandala-runtime/mandala_runtime.compact.compressed.wasm --chain=dev on-runtime-upgrade live --uri wss://mandala.polkawallet.io:443

.PHONY: try-runtime-acala
try-runtime-acala:
	cargo build --release --locked --features with-acala-runtime --features try-runtime --bin acala
	./target/release/acala try-runtime --runtime ./target/release/wbuild/acala-runtime/acala_runtime.compact.compressed.wasm --chain=acala-dev on-runtime-upgrade live --uri wss://acala-polkadot.api.onfinality.io:443/public-ws

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= ${cargo_test} --features with-mandala-runtime --all

.PHONY: insta-test
insta-test: githooks
	INSTA_TEST_RUNNER=nextest SKIP_WASM_BUILD= cargo insta test --features with-mandala-runtime --all --lib --tests
	INSTA_TEST_RUNNER=nextest SKIP_WASM_BUILD= cargo insta test --features with-karura-runtime --all --lib --tests
	INSTA_TEST_RUNNER=nextest SKIP_WASM_BUILD= cargo insta test --features with-acala-runtime --all --lib --tests

.PHONY: test-eth
test-eth: githooks test-evm
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-common --features with-ethereum-compatibility schedule_call_precompile_should_work
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-integration-tests --features with-mandala-runtime --features with-ethereum-compatibility should_not_kill_contract_on_transfer_all
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-integration-tests --features with-mandala-runtime --features with-ethereum-compatibility schedule_call_precompile_should_handle_invalid_input

.PHONY: test-evm
test-evm: githooks
	SKIP_WASM_BUILD= ${cargo_test} -p module-evm -p module-evm-bridge --features tracing
	SKIP_WASM_BUILD= ${cargo_test} --release -p evm-jsontests --features evm-tests

.PHONY: test-runtimes
test-runtimes:
	SKIP_WASM_BUILD= ${cargo_test} --all --lib
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-integration-tests --features=with-mandala-runtime --lib
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-integration-tests --features=with-karura-runtime --lib
	SKIP_WASM_BUILD= ${cargo_test} -p runtime-integration-tests --features=with-acala-runtime --lib

.PHONY: test-ts
test-ts: chainspec-dev
	cd ts-tests && yarn && yarn run build && yarn run test

.PHONY: test-benchmarking
test-benchmarking:
	SKIP_WASM_BUILD= ${cargo_test} --features wasm-bench --package module-evm --package runtime-common
	SKIP_WASM_BUILD= ${cargo_test} --features runtime-benchmarks --all benchmarking

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
	./scripts/build-only-wasm.sh --profile production -p mandala-runtime --features=on-chain-release-build

.PHONY: build-wasm-karura
build-wasm-karura:
	./scripts/build-only-wasm.sh --profile production -p karura-runtime --features=on-chain-release-build

.PHONY: build-wasm-acala
build-wasm-acala:
	./scripts/build-only-wasm.sh --profile production -p acala-runtime --features=on-chain-release-build

.PHONY: build-wasm-mandala-dev
build-wasm-mandala-dev:
	cargo build --profile release -p mandala-runtime --features=genesis-builder

.PHONY: build-wasm-acala-dev
build-wasm-acala-dev:
	cargo build --profile release -p acala-runtime --features=genesis-builder

.PHONY: srtool-build-wasm-mandala
srtool-build-wasm-mandala:
	PACKAGE=mandala-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: srtool-build-wasm-karura
srtool-build-wasm-karura:
	PACKAGE=karura-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: srtool-build-wasm-acala
srtool-build-wasm-acala:
	PACKAGE=acala-runtime PROFILE=production BUILD_OPTS="--features on-chain-release-build,no-metadata-docs" ./scripts/srtool-build.sh

.PHONY: build-wasm-karura-tracing
build-wasm-karura-tracing:
	./scripts/build-only-wasm.sh --profile production -p karura-runtime --features=tracing

.PHONY: build-wasm-acala-tracing
build-wasm-acala-tracing:
	./scripts/build-only-wasm.sh --profile production -p acala-runtime --features=tracing

.PHONY: generate-tokens
generate-tokens:
	./scripts/generate-tokens-and-predeploy-contracts.sh

.PHONY: chainspec-dev
chainspec-dev: build-wasm-mandala-dev
	chain-spec-builder -c chainspecs/dev-base.json create -r ./target/release/wbuild/mandala-runtime/mandala_runtime.compact.compressed.wasm default
	jq -s '.[0] * .[1]' chainspecs/dev-base.json chainspecs/dev.genesis.template.json > chainspecs/dev.json
	chain-spec-builder -c chainspecs/dev-raw-base.json convert-to-raw chainspecs/dev.json
	jq -s '.[0] * .[1]' chainspecs/dev-raw-base.json chainspecs/dev.properties.template.json > chainspecs/dev.json

.PHONY: chainspec-acala-dev
chainspec-acala-dev: build-wasm-acala-dev
	chain-spec-builder -c chainspecs/acala-dev-base.json create -r ./target/release/wbuild/acala-runtime/acala_runtime.compact.compressed.wasm default
	jq -s '.[0] * .[1]' chainspecs/acala-dev-base.json chainspecs/acala-dev.genesis.template.json > chainspecs/acala-dev.json
	chain-spec-builder -c chainspecs/acala-dev-raw-base.json convert-to-raw chainspecs/acala-dev.json
	jq -s '.[0] * .[1]' chainspecs/acala-dev-raw-base.json chainspecs/acala-dev.properties.template.json > chainspecs/acala-dev.json

.PHONY: benchmark-module
benchmark-module:
ifeq ($(words $(pallet)), 0)
	$(error pallet not defined)
endif
ifeq ($(words $(pallet_folder)), 0)
	$(error pallet_folder not defined)
endif
	cargo run $(options) --release --bin=acala --features=runtime-benchmarks --features=with-mandala-runtime -- benchmark pallet --chain=dev --steps=50 --repeat=20 --pallet=$(pallet) --extrinsic="*" --wasm-execution=compiled --heap-pages=4096 --output=./modules/$(pallet_folder)/src/weights.rs --template=./templates/module-weight-template.hbs

.PHONY: benchmark-mandala
benchmark-mandala:
	cargo run $(options) --bin=acala --profile production --features=runtime-benchmarks --features=with-mandala-runtime -- benchmark pallet --chain=dev --steps=50 --repeat=20 '--pallet=$(or $(pallet),*)' '--extrinsic=*' --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/mandala/src/weights/

.PHONY: benchmark-karura
benchmark-karura:
	 cargo run $(options) --bin=acala --profile production --features=runtime-benchmarks --features=with-karura-runtime -- benchmark pallet --chain=karura-dev --steps=50 --repeat=20 '--pallet=$(or $(pallet),*)' '--extrinsic=*' --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/karura/src/weights/

.PHONY: benchmark-acala
benchmark-acala:
	 cargo run $(options) --bin=acala --profile production --features=runtime-benchmarks --features=with-acala-runtime -- benchmark pallet --chain=acala-dev --steps=50 --repeat=20 '--pallet=$(or $(pallet),*)' '--extrinsic=*' --wasm-execution=compiled --heap-pages=4096 --template=./templates/runtime-weight-template.hbs --output=./runtime/acala/src/weights/

.PHONY: benchmark-machine
benchmark-machine:
	 cargo run --profile production --features=with-acala-runtime -- benchmark machine --chain=acala-dev

.PHONY: clippy-fix
clippy-fix:
	CARGO_INCREMENTAL=0 ./orml/scripts/run-clippy.sh --fix -Z unstable-options --broken-code --allow-dirty

.PHONY: bench-evm
bench-evm:
	cargo bench -p runtime-common --features wasm-bench -- json | weight-gen --template ./templates/precompile-weight-template.hbs --output runtime/common/src/precompile/weights.rs
	cargo bench -p module-evm --features wasm-bench -- json | evm-bench/analyze_benches.js runtime/common/src/gas_to_weight_ratio.rs

.PHONY: tools
tools:
	cargo install staging-chain-spec-builder
	cargo install frame-omni-bencher
	cargo install --git https://github.com/paritytech/try-runtime-cli --tag v0.7.0

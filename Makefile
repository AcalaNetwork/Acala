.PHONY: run
run: githooks
	SKIP_WASM_BUILD= cargo run -- --dev -lruntime=debug --instant-sealing

.PHONY: run-eth
run-eth: githooks
	cargo run --features with-ethereum-compatibility -- --dev -lruntime=debug -levm=debug --instant-sealing

.PHONY: toolchain
toolchain:
	./scripts/init.sh

.PHONY: build-full
build-full: githooks
	cargo build

.PHONY: check
check: githooks
	SKIP_WASM_BUILD= cargo check

.PHONY: check-tests
check-tests: githooks
	SKIP_WASM_BUILD= cargo check --tests --all

.PHONY: check-all-runtime
check-all-runtime:
	SKIP_WASM_BUILD= cargo check --tests --all --features with-all-runtime

.PHONY: check-debug
check-debug:
	RUSTFLAGS="-Z macro-backtrace" SKIP_WASM_BUILD= cargo +nightly check

.PHONY: test
test: githooks
	SKIP_WASM_BUILD= cargo test --all

.PHONY: test-all-runtime
test-all-runtime:
	SKIP_WASM_BUILD= cargo test --all --features with-all-runtime

.PHONY: build
build: githooks
	SKIP_WASM_BUILD= cargo build

.PHONY: purge
purge: target/debug/acala
	target/debug/acala purge-chain --dev -y

.PHONY: restart
restart: purge run

target/debug/acala:
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

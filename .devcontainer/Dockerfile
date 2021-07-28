FROM rust
WORKDIR /app

RUN rustup default nightly-2021-06-01 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-06-01

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

COPY . .

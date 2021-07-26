FROM rust as planner
WORKDIR app
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN rustup default nightly-2021-06-01
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

# =============

FROM rust as cacher
WORKDIR app
RUN rustup default nightly-2021-06-01 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-06-01
RUN cargo install cargo-chef
RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json --release

# =============

FROM rust as builder
WORKDIR app

COPY --from=cacher $CARGO_HOME $CARGO_HOME

RUN rustup default nightly-2021-06-01 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-06-01

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

ARG GIT_COMMIT=
ENV GIT_COMMIT=$GIT_COMMIT
ARG BUILD_ARGS

COPY . .

COPY --from=cacher /app/target target

RUN cargo build --release $BUILD_ARGS

# =============

FROM phusion/baseimage:bionic-1.0.0
LABEL maintainer="hello@acala.network"

# RUN mv /usr/share/ca* /tmp && \
# 	rm -rf /usr/share/*  && \
# 	mv /tmp/ca-certificates /usr/share/ && \
# 	useradd -m -u 1000 -U -s /bin/sh -d /acala acala

RUN useradd -m -u 1000 -U -s /bin/sh -d /acala acala

COPY --from=builder /app/target/release/acala /usr/local/bin

# checks
RUN ldd /usr/local/bin/acala && \
	/usr/local/bin/acala --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER acala
EXPOSE 30333 9933 9944

RUN mkdir /acala/data

VOLUME ["/acala/data"]

ENTRYPOINT ["/usr/local/bin/acala"]

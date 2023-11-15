FROM rust:buster as builder
WORKDIR /app

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev protobuf-compiler

COPY . acala

RUN git clone https://github.com/flamegraph-rs/flamegraph.git
RUN (cd flamegraph && cargo build --release)

RUN echo '\
[target.x86_64-unknown-linux-gnu] \n\
linker = "/usr/bin/clang" \n\
rustflags = ["-Clink-arg=-fuse-ld=lld", "-Clink-arg=-Wl,--no-rosegment"] \n\
' >> acala/.config

RUN (cd acala && RUSTFLAGS=-g cargo build --release --locked --features with-acala-runtime --workspace --exclude runtime-integration-tests)

# =============

FROM debian:bookworm as acala
LABEL maintainer="hello@acala.network"

RUN apt update && apt install -y linux-perf timelimit

COPY --from=builder /app/flamegraph/target/release/flamegraph /usr/local/bin
COPY --from=builder /app/acala/target/release/acala /usr/local/bin

RUN ldd /usr/local/bin/flamegraph && \
	/usr/local/bin/flamegraph --version

# checks
RUN ldd /usr/local/bin/acala && \
	/usr/local/bin/acala --version

RUN echo '\
#!/usr/bin/env bash \n\
echo -1 > /proc/sys/kernel/perf_event_paranoid \n\
echo 0 > /proc/sys/kernel/kptr_restrict \n\
DURATION="${DURATION:-30}" \n\
ACALA_ARGS="${ACALA_ARGS:-'--chain=acala --tmp -- --dev'}" \n\
echo Run acala profiling for $DURATION seconds with args $ACALA_ARGS \n\
timelimit -t$DURATION /usr/local/bin/acala $ACALA_ARGS &> /acala/data/log.txt & \n\
/usr/local/bin/flamegraph --no-inline -o /acala/data/flamegraph.svg --pid `pidof acala` &> /dev/null \n\
echo Done. File saved at /acala/data/flamegraph.svg \n\
' >> /entrypoint.sh

VOLUME ["/acala/data"]

ENTRYPOINT ["/bin/bash", "/entrypoint.sh"]

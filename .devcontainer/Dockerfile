# Note: You can use any Debian/Ubuntu based image you want.
FROM mcr.microsoft.com/devcontainers/rust:1

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends cmake pkg-config libssl-dev git clang libclang-dev bash-completion

USER vscode

RUN rustup default nightly-2023-04-15 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2023-04-15

RUN rustup component add rustfmt

RUN git config --global submodule.recurse true

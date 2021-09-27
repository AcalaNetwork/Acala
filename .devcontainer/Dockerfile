# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.192.0/containers/rust/.devcontainer/base.Dockerfile

FROM mcr.microsoft.com/vscode/devcontainers/rust:0-1

# [Optional] Uncomment this section to install additional packages.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends cmake pkg-config libssl-dev git clang libclang-dev

RUN rustup default nightly-2021-06-17 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-06-17

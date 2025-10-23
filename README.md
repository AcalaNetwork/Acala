<p align="center">
  <img src="https://acala.polkawallet-cloud.com/logo/acala-logo-horizontal-gradient.png" width="460">
</p>

<div align="center">


[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/AcalaNetwork/Acala/test.yml?label=Actions&logo=github)](https://github.com/AcalaNetwork/Acala/actions)
[![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/AcalaNetwork/Acala)](https://github.com/AcalaNetwork/Acala/tags)
[![Substrate version](https://img.shields.io/badge/Substrate-2.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.io/)
[![codecov](https://codecov.io/gh/AcalaNetwork/Acala/branch/master/graph/badge.svg?token=ERf7EDgafw)](https://codecov.io/gh/AcalaNetwork/Acala)
[![License](https://img.shields.io/github/license/AcalaNetwork/Acala?color=green)](https://github.com/AcalaNetwork/Acala/blob/master/LICENSE)
 <br />
[![Twitter URL](https://img.shields.io/twitter/url?style=social&url=https%3A%2F%2Ftwitter.com%2FAcalaNetwork)](https://twitter.com/AcalaNetwork)
[![Discord](https://img.shields.io/badge/Discord-gray?logo=discord)](https://discord.gg/xZfRD6rVfJ)
[![Telegram](https://img.shields.io/badge/Telegram-gray?logo=telegram)](https://t.me/AcalaOfficial)
[![Discourse](https://img.shields.io/badge/Forum-gray?logo=discourse)](https://forum.acala.network/)
[![Medium](https://img.shields.io/badge/Medium-gray?logo=medium)](https://medium.com/acalanetwork)

</div>

<!-- TOC -->

- [1. Introduction](#1-introduction)
- [2. Building](#2-building)
	- [NOTE](#note)
- [3. Run](#3-run)
- [4. Development](#4-development)
- [5. Bug Bounty :bug:](#5-bug-bounty-bug)
- [6. Bench Bot](#6-bench-bot)
	- [Generate module weights](#generate-module-weights)
	- [Generate runtime weights](#generate-runtime-weights)
	- [Bench Acala EVM+](#bench-acala-evm)
- [7. Migration testing runtime](#7-migration-testing-runtime)
	- [Try testing runtime](#try-testing-runtime)

<!-- /TOC -->

# 1. Introduction
This project is initiated and facilitated by the Acala Foundation. Acala Foundation nurtures applications in the fields of decentralized finance protocols, particularly those that serve as open finance infrastructures such as stable currency and staking liquidity. The Acala Foundation is founded by [Laminar](https://laminar.one/) and [Polkawallet](https://polkawallet.io/), participants and contributors to the Polkadot ecosystem. The Acala Foundation welcomes more industry participants as it progresses.

# 2. Building

## NOTE

The Acala client node is moved to [acala-node](https://github.com/AcalaNetwork/acala-node). This repo only contains the runtime code. This allow us to decouple the runtime release and client node release.

If you would like to build the client node, please refer to [acala-node](https://github.com/AcalaNetwork/acala-node).

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

You may need additional dependencies, checkout [substrate.io](https://docs.substrate.io/v3/getting-started/installation) for more info

```bash
sudo apt-get install -y git clang curl make libssl-dev llvm libudev-dev protobuf-compiler
```

Make sure you have `submodule.recurse` set to true to make life with submodule easier.

```bash
git config --global submodule.recurse true
```

Install required tools and install git hooks:

```bash
make init
```

# 3. Run

You can start a development chain with:

```bash
make run
```

# 4. Development

To type check:

```bash
make check-all
```

To purge old chain data:

```bash
make purge
```

To purge old chain data and run

```bash
make restart
```

Update ORML

```bash
make update
```

# 5. Bug Bounty :bug:

The Bug Bounty Program includes only on-chain vulnerabilities that can lead to significant economic loss or instability of the network. You check details of the Bug Bounty or Submit a vulnerability here:
https://immunefi.com/bounty/acala/

# 6. Bench Bot
Bench bot can take care of syncing branch with `master` and generating WeightInfos for module or runtime.

## Generate module weights

Comment on a PR `/bench module <module_name>` i.e.: `module_currencies`

Bench bot will do the benchmarking, generate `weights.rs` file and push changes into your branch.

## Generate runtime weights

Comment on a PR `/bench runtime <runtime> <module_name>` i.e.: `/bench runtime mandala module_currencies`.

To generate weights for all modules just pass `*` as `module_name` i.e: `/bench runtime mandala *`

Bench bot will do the benchmarking, generate weights file and push changes into your branch.

## Bench Acala EVM+

Comment on a PR `/bench evm` to benchmark Acala EVM+ and bench bot will generate precompile weights and GasToWeight ratio.

# 7. Migration testing runtime
If modifying the storage, you should test the data migration before upgrading the runtime.

## Try testing runtime

try-runtime on karura

```bash
# Use a live chain to run the migration test.
# Add `-p module_name` can specify the module.
make try-runtime-karura

# Create a state snapshot to run the migration test.
# Add `--pallet module_name` can specify the module.
cargo run --features with-karura-runtime --features try-runtime -- try-runtime --runtime existing create-snapshot --uri wss://karura.api.onfinality.io:443/public-ws karura-latest.snap

# Use a state snapshot to run the migration test.
./target/release/acala try-runtime --runtime ./target/release/wbuild/karura-runtime/karura_runtime.compact.compressed.wasm --chain=karura-dev on-runtime-upgrade snap -s karura-latest.snap
```

try-runtime on acala

```bash
# Use a live chain to run the migration test.
# Add `--pallet module_name` can specify the module.
make try-runtime-acala

# Create a state snapshot to run the migration test.
# Add `-palet module_name` can specify the module.
cargo run --features with-acala-runtime --features try-runtime -- try-runtime --runtime existing create-snapshot --uri wss://acala.api.onfinality.io:443/public-ws acala-latest.snap

# Use a state snapshot to run the migration test.
./target/release/acala try-runtime --runtime ./target/release/wbuild/acala-runtime/acala_runtime.compact.compressed.wasm --chain=acala-dev on-runtime-upgrade snap -s acala-latest.snap
```

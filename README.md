# Introduction
This project is initiated and facilitated by the Acala Foundation. Acala Foundation nurtures and stewards applications in the fields of decentralized finance protocols particularly those that can serve as open finance infrastructures such as stable currency and staking liquidity. The Acala Foundation is founded by [Laminar](https://laminar.one/) and [Polkawallet](https://polkawallet.io/) both of whom are participants and contributors to the Polkadot ecosystem. The Acala Foundation aims to broaden its membership and industry participants as we progress.

# Overview
The significance of cross-chain communication to blockchain is like that of internet to intranet. Polkadot empowers a network of public, consortium and private blockchains, and enables true interoperability, economic and transactional scalability. A cross-chain stablecoin system will:
1. create a sound, stable currency for low cost, borderless value transfer for all chains in the network
2. enable business lending with predictable risk
3. serve as a building block for more open finance services

The Acala Dollar stablecoin (ticker: aUSD) is a multi-collateral-backed cryptocurrency, whose value is stable against US Dollar (aka. 1:1 aUSD to USD soft peg). It is completely decentralized, can use assets from blockchains connected to the Polkadot network including Ethereum and Bitcoin as collaterals, and can be used by any chains (or digital jurisdictions) within the Polkadot network and applications on those chains.

By this nature, it is essential that the Acala Network eventually become community-owned with an economic model that can sustain its development and participation in the Polkadot network, as well as ensure its stability and security. The following section will provide a high level overview of
- aUSD and the Honzon stablecoin protocol
- economic model and initial parachain offering

## aUSD and the Honzon stablecoin protocol
Note: This section is still work in progress, we will update more information as we progress.

Every aUSD is backed in excess by a crypto asset, the mechanism of which is known as over-collateralized debt position (or CDP). This together with a set of incentive and risk management mechanisms, as the core components of the Honzon stablecoin protocol on the Acala Network, ensures stability of the aUSD. The CDP mechanism design is inspired by the first decentralized stablecoin project MakerDAO, which has become the DeFi building block in the Ethereum ecosystem. In addition, the Honzon protocol enables many unique features - native multi-asset support, cross-chain stablecoin capability, automatic liquidation to increase responsiveness to risk, and pluggable oracle and auction house to improve modularity, just to name a few.

The Honzon protocol contains the following components
- Multi-Currency
- Oracle
- Auction Manager
- CDP and CDP Engine
- Prices
- Honzon as interface to other components

## Acala Network Economic Model
Note: This section is still work in progress, we will update more information as we progress.

The Acala Network Token (ACA) has the following utilities, and the value of ACA will accrue with increase usage of the network and revenue from stability fees and liquidation penalties
1. As Network Utility Token: to pay network fee and stability fee
2. As Governance Token: to vote for/against risk parameter and network change proposals
3. As Economic Capital: in case of liquidation without sufficient collaterals

To enable cross-chain functionality, the Acala Network will connect to the Polkadot in one of the three ways: as parathread - pay-as-you-go connection to Polkadot, as parachain - permanent connection for a given period, or as independent chain with a bridge back to Polkadot. Becoming a parachain would be an ideal option to bootstrap the Acala Network, and maximize its benefit and reach to other chains and applications on the Polkadot network. To secure a parachain slot, the Acala Network will require supportive DOT holders to lock their DOTs to bid for the slot - a process known as the Initial Parachain Offering.

ACAs will be offered as reward for those who participated in the Initial Parachain Offering, also to compensate potential lost yield of DOT (for staking).

# Building

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Make sure you have `submodule.recurse` set to true to make life with submodule easier.

```bash
git config --global submodule.recurse true
```

Install required tools and install git hooks:

```bash
make init
```

Build all native code:

```bash
make build
```

# Run

You can start a development chain with:

```bash
make run
```

# Development

To type check:

```bash
make check
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

__Note:__ All build command from Makefile are designed for local development purpose and hence have `SKIP_WASM_BUILD` enabled to speed up build time and use `--execution native` to only run use native execution mode.

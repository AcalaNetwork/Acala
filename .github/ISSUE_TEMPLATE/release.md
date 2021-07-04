---
name: New Release
about: Tracking issue for new releases
title: "Release Checklist: VERSION"
labels: a-release
assignees: qwer951123, ntduan, shaunxw, wangjj9219

---

## New Release Details:

- Scope: (Client only | Runtime only | Full release)
- Network: (Mandala | Karura | Acala | All)
- Client Version: XXX
- Runtime Version: XXX
- Release Branch: [release-karura-XXX](https://github.com/AcalaNetwork/Acala/tree/release-karura-XXX)
  -  Diff: https://github.com/AcalaNetwork/Acala/compare/release-karura-XXX...release-karura-XXX
- Substrate version: [XXX](https://github.com/paritytech/substrate/tree/XXX)
  - Diff: https://github.com/paritytech/substrate/compare/polkadot-vXXX...polkadot-vXXX
- ORML version: [XXX ](https://github.com/open-web3-stack/open-runtime-module-library/tree/XXX)
  - Diff: https://github.com/open-web3-stack/open-runtime-module-library/compare/XXX...XXX
- Cumulus version: [XXX ](https://github.com/paritytech/cumulus/tree/XXX)
  - Diff: https://github.com/paritytech/cumulus/compare/polkadot-vXXX...polkadot-vXXX
- Polkadot version: [XXX ](https://github.com/paritytech/polkadot/tree/XXX)
  - Diff: https://github.com/paritytech/polkadot/compare/release-vXXX...release-vXXX
- srtool details:

```
XXX
```

- subwasm info

```
XXX
```

## Client Release

- [ ] Verify client `Cargo.toml` version has been incremented since the last release.
  - Current version: XXX
  - Last version: XXX
- [ ] Check the new client have run on the network without issue for at lease 12 hours.
- [ ] Check new docker image has been published.
  - [acala/karura-node:XXX](https://hub.docker.com/layers/acala/karura-node/XXX)
- [ ] Check new client is able to sync from scratch
  -  `docker run --rm acala/karura-node:1.1.0 --chain=karura -- --chain=dev`

## Runtime Release

- [ ] Verify `spec_version` has been incremented since the last release.
  - Current version: XXX
  - Last version: XXX
- [ ] Verify completed migrations are removed from any public networks.
- [ ] Review subwasm diff
  - `subwasm diff karura_runtime.compact.compressed.wasm wss://karura-rpc-2.aca-api.network/ws`
- [ ] Verify extrinsic ordering has stayed the same. Bump `transaction_version` if not.
  - [ ] ORML
  - [ ] Substrate
  - [ ] Cumulus
  - [ ] Acala
- [ ] Verify new extrinsics have been correctly whitelisted/blacklisted for proxy filters.
- [ ] Verify benchmarks & weights have been updated for any modified runtime logics.
- [ ] Verify we included all the necessary migrations.
  - [ ] ORML
  - [ ] Substrate
  - [ ] Cumulus
  - [ ] Acala
- [ ] Verify new migrations complete successfully and the runtime state is correctly updated for any public networks.
  - [ ] Verify the execution time to perform runtime upgrade with Karura/Acala onchain data.
- [ ] Ensure WASM is reproducible
  - `make srtool-build-wasm-karura`

## All Releases

- [ ] Check new Github release is created with release logs.

## Post Release

- [ ] Notify Discord announcement channel.
- [ ] Ensure our own nodes are updated
- [ ] Update [wiki](https://wiki.acala.network/karura/integration)

## Compatibility Checklist

### SDK & Tools

- [ ] acala.js
- [ ] txwrapper
- [ ] sidecar
- [ ] acala-subql
- [ ] oracle dispatcher

### dApps

- [ ] polkadot apps
- [ ] Acala dApp

### Wallets

- [ ] Polkawallet
- [ ] Feareless

### Other

- [ ] Exchanges
- [ ] Gauntlet
- [ ] Faucet (for Mandala)

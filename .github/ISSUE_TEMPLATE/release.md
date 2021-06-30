---
name: New Release
about: Tracking issue for new releases
title: "Release Checklist: VERSION"
labels: a-release
assignees: qwer951123, ntduan, shaunxw, wangjj9219

---

## New Release Details:

- Client Version:
- Runtime Version:
- RC Branch:
- Scope: (Client only | Runtime only | Full release)
- Network: (Mandala | Karura | Acala | All)
- Substrate version:
  - Diff:
- ORML version:
  - Diff:
- Cumulus version:
  - Diff:
- Polkadot version:
  - Diff:
- wasm srtool details:

## Client Release

- [ ] Verify client `Cargo.toml` version has been incremented since the last release.
  - Current version:
  - Last version:
- [ ] Check the new client have run on the network without issue for at lease 12 hours.
- [ ] Check new docker image has been published.

## Runtime Release

- [ ] Verify `spec_version` has been incremented since the last release.
  - Current version:
  - Last version:
- [ ] Verify completed migrations are removed from any public networks.
- [ ] Review [subwasm diff](#subwasm-diff)
- [ ] Verify extrinsic ordering has stayed the same. Bump `transaction_version` if not.
  - [ ] ORML
  - [ ] Substrate
  - [ ] Cumulus
  - [ ] Acala
- [ ] Verify new extrinsics have been correctly whitelisted/blacklisted for proxy filters.
- [ ] Verify benchmarks & weights have been updated for any modified runtime logics.
- [ ] Verify SDK & dApp compatibility with new runtime.
- [ ] Verify we included all the necessary migrations.
  - [ ] ORML
  - [ ] Substrate
  - [ ] Cumulus
  - [ ] Acala
- [ ] Verify new migrations complete successfully and the runtime state is correctly updated for any public networks.
  - [ ] Verify the execution time to perform runtime upgrade with Karura/Acala onchain data.
- [ ] Verify the release log includes the hash of the wasm runtime and it is reproducible using `srtool`.

## All Releases

- [ ] Check new Github release is created with release logs.

## Post Release

- [ ] Notify Discord announcement channel.

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

# Notes

## subwasm diff

[subwasm](https://github.com/chevdor/subwasm/) can be used to verify wasm and check diff

```bash
$ subwasm karura_runtime.compact.compressed.wasm wss://karura-rpc-2.aca-api.network/ws
```

Review the output and make sure 

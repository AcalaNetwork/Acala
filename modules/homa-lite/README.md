# Homa Lite Module

## Overview
The Homa Lite module allows users to upload some Staking currency, and mint some Liquid currency.

### Signed origin dispatchable calls
* Request Mint: Request to upload Staking currency to the relaychain via XCM transfer.  
* Claim: After a batch has been processed, the user can claim the Liquid currency minted from the Staking currency uploaded.

### Governance origin dispatchable calls 
* issue: For a batch, set the Total issuance of the Staking currency. This will be used to calculate the exchange rate for the Liquid currency.

## Test
Currently, the Homa Lite module is integrated into the Mandala Runtime. 
The Staking currency is set as "KSM"
The Liquid currency is set as "LKSM"

Homa-lite uses XCM transfer to upload Staking currency into the Relaychain. Therefore a setup that allows successful XCM transfer to the relaychain is required for the Homa-lite module to work.

### Local node
1. Pull the Master branch of the Acala codebase
2. Follow the README.md to setup local Relaychain and parachains.

   This should launch some local test nodes running Karura(parachain) and Rococo(relaychain)
3. Open a new web browser, go to `https://polkadot.js.org/apps/#/explorer`
4. Connect to a parachain node.
5. On the top bar, select `Settings` -> `Developer`
6. Add the following metadata into the field:
``` JSON
{
  "TotalIssuanceInfo": {
    "staking_total": "Balance",
    "liquid_total": "Balance"
  }
}
```
7. You can now send Extrinsic to the Homa Lite Module for testing.

### Reference on how  to use the Pokadot.js app
#### To submit an extrinsics as ROOT
* Open the Developer -> Extrinsics tab. Select `sudo` -> `sudo(call)`
* Ensure ALICE signs the transaction. In the `make run` test chain, ALICE is the root.
* Select the module and extrinsic as you would otherwise.

#### To mint new Tokens to an account
* Open the Developer -> Extrinsics tab. Select `sudo` -> `sudo(call)` -> `Currencies` -> `updateBalance` -> 
* Select User -> Token -> Select Token symbol -> Select amount.
* Note: the amount needs to be multiplied by 10^12. i.e., 1 KSM should be entered as 1000000000000

#### To query a chain state:
* Open the Developer -> Chain State
* Select the module, and the storage to be queried

### Workflow: Minting Liquid from a fresh chain.
#### First we need to set up the chain state.
Use SUDO to:
1. Mint 1_000_000 Staking to Alice
2. Mint 1_000_000 Staking to Bob
3. Mint 1_000_000_000 Liquid to Ferdie
4. Call set_staking_currency_cap to set a large enough cap.

#### Use the normal Extrinsic to Request Mint
5. Request to mint 1000 as Alice
6. Request to mint 2000 as Bob

#### Use Sudo to Issue
7. Issue Liquid currency for Batch 0, use 1_000_000 as the total Issuance for Staking
This will make the Staking to Liquid ratio to be 1:1000
8. Check the chain stain: `HomaLite` -> `batchTotalIssuanceInfo` -> `batch 0` should have an entry.
the Staking total issuance should be 1_000_000, and the Liquid total issuance should be 1_000_000_000

#### Use the normal Extrinsic to claim the liquid currency
9. Use Alice (It doesn't matter who claims. We use Alice since she has lots of ACA) to claim for Alice for Batch 0
10. Claim for Bob for batch 0

#### Now verify the liquid currency has been minted
11. Use Chain state query to check: Tokens -> accounts -> Alice -> Token -> Liquid should now have 1_000_000 Liquid
12. Verify Bob should have  2_000_000 Liquid.
Formula for amount of Liquid currency to mint is: amount * Liquid Total / StakingTotal

#### Note: Currency configurations
For Mandala network (default with `make run`):
* Staking currency: DOT
* Liquid currency: LDOT

For Karura network:
* Staking currency: KSM
* Liquid currency: LKSM
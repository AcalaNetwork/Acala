# Homa Lite Module

## Overview
The Homa Lite module allows users to upload some Staking currency into the relaychain, and mint some Liquid currency on the parachain.

The amount exchanged is calculated as the following:
``` liquid_to_mint = ( (staked_amount - MintFee) * liquid_total / staked_total ) * (1 - MaxRewardPerEra) ```

### Signed origin dispatchable calls
* Mint: Upload Staking currency to the relaychain via XCM transfer, and mint some Liquid currency.

### Governance origin dispatchable calls 
* set_total_staking_currency: Set the Total amount of the Staking currency. This will be used to calculate the exchange rate for the Liquid currency.
* set_minting_cap: Sets the maximum amount of staking currency allowed to be used to mint Liqid currency.

#### Runtime Integration
Currently the Homa-lite module is integrated into both the Mandala and the Karura rnutime.
For Mandala network (default with `make run`):
* Staking currency: DOT
* Liquid currency: LDOT

For Karura network:
* Staking currency: KSM
* Liquid currency: LKSM

## Test
Homa-lite uses XCM transfer to upload Staking currency into the RelayChain. Therefore a setup that allows successful XCM transfer to the relaychain is required for full end-to-end test of the Homa-lite module.

### Local node
1. Pull the Master branch of the Acala codebase
2. Follow the README.md to setup local RelayChain and parachains.

   This should launch some local test nodes running Karura(parachain) and Rococo(relaychain)
3. Open a new web browser, go to `https://polkadot.js.org/apps/#/explorer`
4. Connect to a parachain node.
5. You can now send Extrinsics to the Homa Lite Module for testing.

### Reference on how to use the Pokadot.js app
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

#### Use Sudo to Issue
1. Set the total amount for the staking currency. Use 1_000_000, as this will make the Staking to Liquid ratio to be 1:1000
2. Check the chain stain: `HomaLite` -> `TotalStakingCurrency` should have the right amount.

#### Use the normal Extrinsic to Request Mint
1. Mint 1000 as Alice
2. Verify some amount of liquid currency is minted into Alice's account.
3. For full e2e testing, also verify the correct amount of staking currency is received on the relaychain.
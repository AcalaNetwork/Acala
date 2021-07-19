# Ecosystem - Starport Module

## Overview
The Starport module is used to connect with Compound Finance.

### User Functions
Users can:
* Uploading Assets: User can lock assets native to Acala to "upload" them onto the Compound chain.
* Downloading Assets: User can unlock/download assets back from Compound chain back to Acala, through the construction of a Gateway Notice.

### Administrative Functions 
Through the use of Gateway Notice, this module currently supports the following:
* Setting the Supply Cap: Only assets with sufficient Supply Cap can be uploaded.
* Change Gateway Authorities: These authorities are used to verify the authenticity of Gateway Notices.
  Initially set by GenesisConfig, these can only be updated through Notice.
* Setting the Future Yield for Cash tokens: Sets the interest rate for the Cash token while they are on Acala chain.

## Test
Currently the Starport module is integrated into the Mandala Runtime. 

### Local node
1. Pull the Master branch of the Acala codebase
2. Open a console, run the following commands:
   ```shell
   make init
   make run
    ```
   This should launch a local test Mandala node
3. Open a new web browser, go to `https://polkadot.js.org/apps/#/explorer`
4. On the left top corner, select `DEVELOPMENT` -> `Local Node`. Click "Switch" to confirm connection.
5. On the top bar, select `Settings` -> `Developer`
6. Copy the following metadata into the field:
``` JSON
{
   "TokenSymbol": {
       "_enum": {
           "ACA": 0,
           "AUSD": 1,
           "DOT": 2,
           "LDOT": 3,
           "RENBTC": 4,
           "KAR": 128,
           "KUSD": 129,
           "KSM": 130,
           "LKSM": 131,
           "CASH": 140
       }
   },
   "CashYieldIndex": "u128",
   "GatewayNoticePayload": {
       "_enum": {
           "SetSupplyCap": "(CurrencyId, Balance)",
           "ChangeAuthorities": "Vec<CompoundAuthoritySignature>",
           "Unlock": "(CurrencyId, Balance, AccountId)",
           "SetFutureYield": "(Balance, CashYieldIndex, Moment)"
       }
   },
   "GatewayNotice": {
      "id": "u64",
      "payload": "GatewayNoticePayload"
   },
   "CompoundAuthoritySignature": "AccountId"
}
```
7. You can now send Extrinsics to the Starport Module for testing, or query the chain state.

### Example 1: Lock Tokens / Upload Assets 
To lock tokens, we must first set the supply cap of that token.
1. Open the Extrinsics tab. Select `Starport` -> `invoke`
2. Select `SetSupplyCap` -> Select your Token of choice -> Add the amount
3. For `Signatures`, add `Alice`, because `Alice` is the default Gateway Authority.
4. Submit the transaction.
   
We can now lock/upload the token of your choice.
1. In the Extrinsics Tab, select `Starport` -> `lock` -> Select your token e.g. DOT and set an amount
2. Submit the transaction.
3. If you go to the "Explorer" tab, you should see the correct events have been deposited.

### Example 2: Unlock Tokens / Download Assets 
1. Open the Extrinsics tab. Select `Starport` -> `invoke`
2. Select `payload` as `Unlock`
3. Then select token e.g. CASH and set an amount to download
4. 3. For `Signatures`, add `Alice`, as `Alice` is the default Gateway Authority.
5. Submit the transaction.
6. You should see the correct events deposited in the `Explorer` tab.
# Acala Bug Bounty

## Program overview
Acala is the decentralized finance network and liquidity hub of Polkadot. It’s a layer-1 smart contract platform that’s scalable, Ethereum-compatible, and optimized for DeFi with built-in liquidity and ready-made financial applications. With its trustless exchange, decentralized stablecoin (aUSD), DOT Liquid Staking (LDOT), and EVM+, Acala lets developers access the best of Ethereum and the full power of Substrate.
Karura is a "canary network" for Acala, which means it's an early, unaudited release of the code and holds real economic value. Karura’s all-in-one DeFi hub for Kusama gives users a taste of a decentralized swap, stablecoin minting, liquid staking, earning rewards, and more – all with micro gas fees.
Founded by the Acala Foundation, Karura is a scalable, EVM-compatible network optimized for DeFi. The platform offers a suite of financial applications including: a trustless staking derivative (liquid KSM), a multi-collateralized stablecoin backed by cross-chain assets (kUSD), and an AMM DEX – all with micro gas fees that can be paid in any token.
For developers, Karura is a proving ground for protocol upgrades and a place to experiment with new DeFi protocols and on-chain governance. 
For more information about Acala, please visit https://acala.network/.

This bug bounty program is focused on Karura and is focused on preventing:
- Transaction/consensus manipulation, 
- Double-spending, 
- Unauthorized token minting, 
- Governance compromise, 
- Getting access to an identity that can lead to unauthorized access to system’s or user’s assets. 
- Blocking or modifying processes for governance or users from performing their tasks, generating not handled on-chain errors. 
- Putting on-chain data into an unexpected state without interrupting the system or users from performing their tasks, e.g. generating redundant events, logs, etc.

### You can [Submit a Bug here](https://immunefi.com/bounty/acala/).



## Rewards by threat level
Rewards are distributed according to the impact of the vulnerability based on the following severity scale:

- **Critical**: transaction/consensus manipulation, double-spending, unauthorized token minting, governance compromise, getting access to an identity that can lead to unauthorized access to system’s or user’s assets. 
- **High**: blocking or modifying processes for governance or users from performing their tasks, generating not handled on-chain errors. These actions can lead to blocking users or governance from accessing their assets or performing system functions.
- **Medium**: Putting on-chain data into an unexpected state without interrupting the system or users from performing their tasks, e.g. generating redundant events, logs, etc.


Rewards by Severety Level:



| Level | Reward |
| -------- | -------- | 
| **Critical**     | Up to USD 1 000 000     |
| **High**     | Up to USD  50 000     |
| **Medium**     | USD   10 000     |


					
					
					


Critical vulnerabilities involving a direct loss of user funds, double spending, or the minting of tokens are capped at 10% of the economic damage, taking primarily into consideration the funds at risk or the amount of tokens that can be minted but also branding and PR considerations, at the discretion of the team. However, there is a minimum reward of **USD 50,000**. Consensus manipulation or governance compromise results in the full **USD 1,000,000**. 

The addition of a PoC and a suggestion for a fix is not required, but its addition may be grounds for a bonus provided by the team at its discretion.

### A reward can only be provided if:

- The Bug wasn't reported before.
- The Bounty Hunter does not disclose the Bug to other parties or publicity until it's fixed by the Karura Team.
- The Hunter didn't exploit the vulnerability or allow anyone else to profit from it.
- The Hunter reports a Bug without any additional conditions or threats.
- The investigation was NOT conducted with Ineligible methods or Prohibited Activities, defined in this document.
- The Hunter should reply to our additional questions regarding the reproduction of the reported bug (if they follow) within a reasonable time.
- When duplicate bug reports occur, we reward only the first one if it's provided with enough information for reproduction.
- When multiple vulnerabilities are caused by one underlying issue, we will reward only the first reported.
- The vulnerability is found in runtime pallet of Karura (no tests, or modules that aren’t in runtime, e.g. live, can be considered as vulnerability)

Payouts are handled by the Acala team directly and are denominated in USD. However, payouts are done in KUSD.


## Assets in Scope 



| Target | Type |
| -------- | -------- |
| https://github.com/AcalaNetwork/Acala      |   Blockchain - Main Network   |
| https://github.com/open-web3-stack/open-runtime-module-library  | Blockchain - Open Runtime Module Library |


**Only** code involving runtime pallets of Karura are considered as in-scope of the bug bounty program. Modules that are not in runtime pallets like tests, those under development, and those that are not live, are considered as out-of-scope of the bug bounty program. 

All code of Acala can be found at https://github.com/AcalaNetwork/. However, only those in the Assets in Scope table are considered as in-scope of the bug bounty program.


### Impacts in Scope

Only the following impacts are accepted within this bug bounty program. All other impacts are not considered as in-scope, even if they affect something in the assets in scope table.

- Transaction/consensus manipulation, 
- Double-spending, 
- Unauthorized token minting, 
- Governance compromise, 
- Getting access to an identity that can lead to unauthorized access to system’s or user’s assets. 
- Blocking or modifying processes for governance or users from performing their tasks, generating not handled on-chain errors. 
- Putting on-chain data into an unexpected state without interrupting the system or users from performing their tasks, e.g. generating redundant events, logs, etc.



### Prioritized vulnerabilities

 We are especially interested in receiving and rewarding vulnerabilities that lead to the impacts stated in the Impacts in Scope section


## Out of Scope & Rules 

### The following vulnerabilities are excluded from the rewards for this bug bounty program:

- Attacks that the reporter has already exploited themselves, leading to damage
- Attacks requiring access to leaked keys/credentials
- Attacks requiring access to privileged addresses (governance, strategist)
- DDOS attack
- Denial of service attacks
- Spamming
- Any physical attacks against Karura property, or employees
- Phishing or other social engineering attacks against our Karura’s employees

### The following activities are prohibited by this bug bounty program:

- Any testing with mainnet or public testnet contracts; all testing should be done on private testnets
- Any testing with pricing oracles or third party smart contracts
- Attempting phishing or other social engineering attacks against our employees and/or customers
- Any testing with third party systems and applications (e.g. browser extensions) as well as websites (e.g. SSO providers, advertising networks)
- Any denial of service attacks
- Automated testing of services that generates significant amounts of traffic
- Public disclosure of an unpatched vulnerability in an embargoed bounty

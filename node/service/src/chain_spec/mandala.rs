// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use acala_primitives::{AccountId, Balance, Nonce, TokenSymbol};
use hex_literal::hex;
use module_evm::GenesisAccount;
use sc_chain_spec::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{bytes::from_hex, crypto::UncheckedInto, sr25519, Bytes, H160};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{traits::Zero, FixedPointNumber, FixedU128};
use sp_std::{collections::btree_map::BTreeMap, str::FromStr};

use crate::chain_spec::{get_account_id_from_seed, get_authority_keys_from_seed, Extensions, TELEMETRY_URL};

pub type ChainSpec = sc_service::GenericChainSpec<mandala_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 2000;

/// Development testnet config (single validator Alice), non-parachain
pub fn dev_testnet_config() -> Result<ChainSpec, String> {
	dev_testnet_config_from_chain_id("mandala-dev")
}

/// Parachain development testnet config (single collator Alice)
pub fn parachain_dev_testnet_config() -> Result<ChainSpec, String> {
	dev_testnet_config_from_chain_id("mandala-pc-dev")
}

fn dev_testnet_config_from_chain_id(chain_id: &str) -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	TokenSymbol::get_info().iter().for_each(|(symbol_name, decimals)| {
		token_symbol.push(symbol_name.to_string());
		token_decimals.push(*decimals);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());

	let wasm_binary = mandala_runtime::WASM_BINARY.unwrap_or_default();

	Ok(ChainSpec::from_genesis(
		"Mandala Dev",
		chain_id,
		ChainType::Development,
		move || {
			testnet_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![get_authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					// EVM dev accounts
					// 5EMjsczjoEZaNbWzoXDcZtZDSHN1SLmu4ArJcEJVorNDfUH3
					// mnemonic: 'fox sight canyon orphan hotel grow hedgehog build bless august weather swarm',
					hex!["65766d3a75e480db528101a381ce68544611c169ad7eb3420000000000000000"].into(),
					hex!["65766d3a0085560b24769dac4ed057f1b2ae40746aa9aab60000000000000000"].into(),
					hex!["65766d3a0294350d7cf2c145446358b6461c1610927b3a870000000000000000"].into(),
					hex!["65766d3aa76f290c490c70f2d816d286efe47fd64a35800b0000000000000000"].into(),
					hex!["65766d3a4f9c798553d207536b79e886b54f169264a7a1550000000000000000"].into(),
					hex!["65766d3aa1b04c9cbb449d13c4fc29c7e6be1f810e6f35e90000000000000000"].into(),
					hex!["65766d3aad9fbd38281f615e7df3def2aad18935a9e0ffee0000000000000000"].into(),
					hex!["65766d3a0783094aadfb8ae9915fd712d28664c8d7d26afa0000000000000000"].into(),
					hex!["65766d3ae860947813c207abf9bf6722c49cda515d24971a0000000000000000"].into(),
					hex!["65766d3a8bffc896d42f07776561a5814d6e4240950d6d3a0000000000000000"].into(),
				],
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	TokenSymbol::get_info().iter().for_each(|(symbol_name, decimals)| {
		token_symbol.push(symbol_name.to_string());
		token_decimals.push(*decimals);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());

	let wasm_binary = mandala_runtime::WASM_BINARY.ok_or("Dev runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Local",
		"local",
		ChainType::Local,
		move || {
			testnet_genesis(
				wasm_binary,
				vec![
					get_authority_keys_from_seed("Alice"),
					get_authority_keys_from_seed("Bob"),
				],
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
				],
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

pub fn latest_mandala_testnet_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	TokenSymbol::get_info().iter().for_each(|(symbol_name, decimals)| {
		token_symbol.push(symbol_name.to_string());
		token_decimals.push(*decimals);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());

	let wasm_binary = mandala_runtime::WASM_BINARY.ok_or("Mandala runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Acala Mandala TC7",
		"mandala-dev-tc7",
		ChainType::Live,
		// SECRET="..."
		// ./target/debug/subkey inspect "$SECRET//acala//root"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//oracle"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//1//validator"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//1//babe"
		// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//1//grandpa"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//2//validator"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//2//babe"
		// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//2//grandpa"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//3//validator"
		// ./target/debug/subkey --sr25519 inspect "$SECRET//acala//3//babe"
		// ./target/debug/subkey --ed25519 inspect "$SECRET//acala//3//grandpa"
		move || {
			mandala_genesis(
				wasm_binary,
				vec![
					(
						// 5CLg63YpPJNqcyWaYebk3LuuUVp3un7y1tmuV3prhdbnMA77
						hex!["0c2df85f943312fc853059336627d0b7a08669629ebd99b4debc6e58c1b35c2b"].into(),
						hex!["0c2df85f943312fc853059336627d0b7a08669629ebd99b4debc6e58c1b35c2b"].into(),
						hex!["21b5a771b99ef0f059c476502c018c4b817fb0e48858e95a238850d2b7828556"].unchecked_into(),
						hex!["948f15728a5fd66e36503c048cc7b448cb360a825240c48ff3f89efe050de608"].unchecked_into(),
					),
					(
						// 5FnLzAUmXeTZg5J9Ao5psKU68oA5PBekXqhrZCKDbhSCQi88
						hex!["a476c0050065dafac1e9ff7bf602fe628ceadacf67650f8317554bd571b73507"].into(),
						hex!["a476c0050065dafac1e9ff7bf602fe628ceadacf67650f8317554bd571b73507"].into(),
						hex!["77f3c27e98da7849ed0749e1dea449321a4a5a36a1dccf3f08fc0ab3af24c62e"].unchecked_into(),
						hex!["b4f5713322656d29930aa89efa5509554a36c40fb50a226eae0f38fc1a6ceb25"].unchecked_into(),
					),
					(
						// 5Gn5LuLuWNcY21Vue4QcFFD3hLvjQY3weMHXuEyejUbUnArt
						hex!["d07e538fee7c42be9b2627ea5caac9a30f1869d65af2a19df70138d5fcc34310"].into(),
						hex!["d07e538fee7c42be9b2627ea5caac9a30f1869d65af2a19df70138d5fcc34310"].into(),
						hex!["c5dfcf68ccf1a64ed4145383e4bbbb8bbcc50f654d87187c39df2b88a9683b7f"].unchecked_into(),
						hex!["4cc54799f38715771605a21e8272a7a1344667e4681611988a913412755a8a04"].unchecked_into(),
					),
				],
				// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
				hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
				vec![
					// 5F98oWfz2r5rcRVnP9VCndg33DAAsky3iuoBSpaPUbgN9AJn
					hex!["8815a8024b06a5b4c8703418f52125c923f939a5c40a717f6ae3011ba7719019"].into(),
					// 5Fe3jZRbKes6aeuQ6HkcTvQeNhkkRPTXBwmNkuAPoimGEv45
					hex!["9e22b64c980329ada2b46a783623bcf1f1d0418f6a2b5fbfb7fb68dbac5abf0f"].into(),
				],
			)
		},
		vec![
			"/ip4/54.254.37.221/tcp/30333/p2p/12D3KooWNUPp9ervpypz95DCMHfb3CAbQdfrmmBbYehUaJsFvRvT"
				.parse()
				.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("mandala-dev-tc7"),
		Some(properties),
		Extensions {
			relay_chain: "dev".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

pub fn mandala_testnet_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/mandala-dist.json")[..])
}

fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, AuraId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> mandala_runtime::GenesisConfig {
	use mandala_runtime::{
		dollar, get_all_module_accounts, BalancesConfig, CdpEngineConfig, CdpTreasuryConfig, CollatorSelectionConfig,
		DexConfig, EVMConfig, EnabledTradingPairs, FinancialCouncilMembershipConfig, GeneralCouncilMembershipConfig,
		HomaCouncilMembershipConfig, IndicesConfig, NativeTokenExistentialDeposit, OperatorMembershipAcalaConfig,
		OrmlNFTConfig, ParachainInfoConfig, PolkadotXcmConfig, RenVmBridgeConfig, SessionConfig, SessionDuration,
		SessionKeys, SessionManagerConfig, StakingPoolConfig, StarportConfig, SudoConfig, SystemConfig,
		TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig, ACA, AUSD, DOT, LDOT, RENBTC,
	};

	let existential_deposit = NativeTokenExistentialDeposit::get();

	let initial_balance: u128 = 10_000_000 * dollar(ACA);
	let initial_staking: u128 = 100_000 * dollar(ACA);

	let evm_genesis_accounts = evm_genesis();
	let balances = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), initial_staking + dollar(ACA))) // bit more for fee
		.chain(endowed_accounts.iter().cloned().map(|k| (k, initial_balance)))
		.chain(
			get_all_module_accounts()
				.iter()
				.map(|x| (x.clone(), existential_deposit)),
		)
		.fold(
			BTreeMap::<AccountId, Balance>::new(),
			|mut acc, (account_id, amount)| {
				if let Some(balance) = acc.get_mut(&account_id) {
					*balance = balance
						.checked_add(amount)
						.expect("balance cannot overflow when building genesis");
				} else {
					acc.insert(account_id.clone(), amount);
				}
				acc
			},
		)
		.into_iter()
		.collect::<Vec<(AccountId, Balance)>>();

	mandala_runtime::GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		starport: StarportConfig {
			initial_authorities: vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
		},
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig { balances },
		sudo: SudoConfig { key: root_key.clone() },
		general_council: Default::default(),
		general_council_membership: GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		financial_council: Default::default(),
		financial_council_membership: FinancialCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		homa_council: Default::default(),
		homa_council_membership: HomaCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		operator_membership_acala: OperatorMembershipAcalaConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		democracy: Default::default(),
		treasury: Default::default(),
		tokens: TokensConfig {
			balances: endowed_accounts
				.iter()
				.flat_map(|x| vec![(x.clone(), DOT, initial_balance), (x.clone(), AUSD, initial_balance)])
				.collect(),
		},
		vesting: VestingConfig { vesting: vec![] },
		cdp_treasury: CdpTreasuryConfig {
			expected_collateral_auction_size: vec![
				(DOT, dollar(DOT)), // (currency_id, max size of a collateral auction)
				(RENBTC, dollar(RENBTC)),
			],
		},
		cdp_engine: CdpEngineConfig {
			collaterals_params: vec![
				(
					DOT,
					Some(FixedU128::zero()),                             // interest rate per sec for this collateral
					Some(FixedU128::saturating_from_rational(150, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(10, 100)),  // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(150, 100)), // required liquidation ratio
					10_000_000 * dollar(AUSD),                           // maximum debit value in aUSD (cap)
				),
				(
					LDOT,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(180, 100)),
					10_000_000 * dollar(AUSD),
				),
				(
					RENBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(150, 100)),
					10_000_000 * dollar(AUSD),
				),
			],
			global_interest_rate_per_sec: FixedU128::saturating_from_rational(
				1_547_126_000u128,
				1_000_000_000_000_000_000u128,
			), /* 5% APR */
		},
		evm: EVMConfig {
			accounts: evm_genesis_accounts,
			treasury: root_key,
		},
		staking_pool: StakingPoolConfig {
			staking_pool_params: module_staking_pool::Params {
				target_max_free_unbonded_ratio: FixedU128::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: FixedU128::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: FixedU128::saturating_from_rational(2, 100),
				unbonding_to_free_adjustment: FixedU128::saturating_from_rational(1, 1000),
				base_fee_rate: FixedU128::saturating_from_rational(2, 100),
			},
		},
		dex: DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			#[cfg(feature = "runtime-benchmarks")]
			initial_added_liquidity_pools: vec![],
			#[cfg(not(feature = "runtime-benchmarks"))]
			initial_added_liquidity_pools: vec![(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(
						mandala_runtime::TradingPair::from_currency_ids(AUSD, DOT).unwrap(),
						(1_000_000 * dollar(AUSD), 2_000_000 * dollar(DOT)),
					),
					(
						mandala_runtime::TradingPair::from_currency_ids(AUSD, ACA).unwrap(),
						(1_000_000 * dollar(AUSD), 2_000_000 * dollar(ACA)),
					),
				],
			)],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		ren_vm_bridge: RenVmBridgeConfig {
			ren_vm_public_key: hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"],
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _, _, _)| acc).collect(),
			candidacy_bond: initial_staking,
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _, _, aura)| {
					(
						acc.clone(),          // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},
		session_manager: SessionManagerConfig {
			session_duration: SessionDuration::get(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: PolkadotXcmConfig {
			safe_xcm_version: Some(2),
		},
	}
}

fn mandala_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, AuraId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
) -> mandala_runtime::GenesisConfig {
	use mandala_runtime::{
		cent, dollar, get_all_module_accounts, BalancesConfig, CdpEngineConfig, CdpTreasuryConfig,
		CollatorSelectionConfig, DexConfig, EVMConfig, EnabledTradingPairs, FinancialCouncilMembershipConfig,
		GeneralCouncilMembershipConfig, HomaCouncilMembershipConfig, IndicesConfig, NativeTokenExistentialDeposit,
		OperatorMembershipAcalaConfig, OrmlNFTConfig, ParachainInfoConfig, PolkadotXcmConfig, RenVmBridgeConfig,
		SessionConfig, SessionDuration, SessionKeys, SessionManagerConfig, StakingPoolConfig, StarportConfig,
		SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig, ACA, AUSD, DOT,
		LDOT, RENBTC,
	};

	let existential_deposit = NativeTokenExistentialDeposit::get();

	let initial_balance: u128 = 1_000_000 * dollar(ACA);
	let initial_staking: u128 = 100_000 * dollar(ACA);

	let evm_genesis_accounts = evm_genesis();
	let balances = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), initial_staking + dollar(ACA))) // bit more for fee
		.chain(endowed_accounts.iter().cloned().map(|k| (k, initial_balance)))
		.chain(
			get_all_module_accounts()
				.iter()
				.map(|x| (x.clone(), existential_deposit)),
		)
		.fold(
			BTreeMap::<AccountId, Balance>::new(),
			|mut acc, (account_id, amount)| {
				if let Some(balance) = acc.get_mut(&account_id) {
					*balance = balance
						.checked_add(amount)
						.expect("balance cannot overflow when building genesis");
				} else {
					acc.insert(account_id.clone(), amount);
				}
				acc
			},
		)
		.into_iter()
		.collect::<Vec<(AccountId, Balance)>>();

	mandala_runtime::GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		starport: StarportConfig {
			initial_authorities: vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
		},
		indices: IndicesConfig { indices: vec![] },
		balances: BalancesConfig { balances },
		sudo: SudoConfig { key: root_key.clone() },
		general_council: Default::default(),
		general_council_membership: GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		financial_council: Default::default(),
		financial_council_membership: FinancialCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		homa_council: Default::default(),
		homa_council_membership: HomaCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		operator_membership_acala: OperatorMembershipAcalaConfig {
			members: endowed_accounts,
			phantom: Default::default(),
		},
		democracy: Default::default(),
		treasury: Default::default(),
		tokens: TokensConfig {
			balances: vec![(root_key.clone(), DOT, initial_balance)],
		},
		vesting: VestingConfig { vesting: vec![] },
		cdp_treasury: CdpTreasuryConfig {
			expected_collateral_auction_size: vec![
				(DOT, dollar(DOT)), // (currency_id, max size of a collateral auction)
				(RENBTC, 5 * cent(RENBTC)),
			],
		},
		cdp_engine: CdpEngineConfig {
			collaterals_params: vec![
				(
					DOT,
					Some(FixedU128::zero()),                             // interest rate per sec for this collateral
					Some(FixedU128::saturating_from_rational(105, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(3, 100)),   // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(110, 100)), // required liquidation ratio
					10_000_000 * dollar(AUSD),                           // maximum debit value in aUSD (cap)
				),
				(
					LDOT,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(120, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(130, 100)),
					10_000_000 * dollar(AUSD),
				),
				(
					RENBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(110, 100)),
					Some(FixedU128::saturating_from_rational(4, 100)),
					Some(FixedU128::saturating_from_rational(115, 100)),
					10_000_000 * dollar(AUSD),
				),
			],
			global_interest_rate_per_sec: FixedU128::saturating_from_rational(
				1_547_126_000u128,
				1_000_000_000_000_000_000u128,
			), /* 5% APR */
		},
		evm: EVMConfig {
			accounts: evm_genesis_accounts,
			treasury: root_key,
		},
		staking_pool: StakingPoolConfig {
			staking_pool_params: module_staking_pool::Params {
				target_max_free_unbonded_ratio: FixedU128::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: FixedU128::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: FixedU128::saturating_from_rational(2, 100),
				unbonding_to_free_adjustment: FixedU128::saturating_from_rational(1, 1000),
				base_fee_rate: FixedU128::saturating_from_rational(2, 100),
			},
		},
		dex: DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		ren_vm_bridge: RenVmBridgeConfig {
			ren_vm_public_key: hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"],
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _, _, _)| acc).collect(),
			candidacy_bond: initial_staking,
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _, _, aura)| {
					(
						acc.clone(),          // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},
		session_manager: SessionManagerConfig {
			session_duration: SessionDuration::get(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: PolkadotXcmConfig {
			safe_xcm_version: Some(2),
		},
	}
}

/// Returns `evm_genesis_accounts`
pub fn evm_genesis() -> BTreeMap<H160, GenesisAccount<Balance, Nonce>> {
	let contracts_json = &include_bytes!("../../../../predeploy-contracts/resources/bytecodes.json")[..];
	let contracts: Vec<(String, String, String)> = serde_json::from_slice(contracts_json).unwrap();
	let mut accounts = BTreeMap::new();
	for (_, address, code_string) in contracts {
		let account = GenesisAccount {
			nonce: 0u32,
			balance: 0u128,
			storage: BTreeMap::new(),
			code: if code_string.len().is_zero() {
				vec![]
			} else {
				Bytes::from_str(&code_string).unwrap().0
			},
		};

		let addr = H160::from_slice(
			from_hex(address.as_str())
				.expect("predeploy-contracts must specify address")
				.as_slice(),
		);
		accounts.insert(addr, account);
	}
	accounts
}

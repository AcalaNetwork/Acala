use acala_primitives::AccountId;
use hex_literal::hex;
use sc_chain_spec::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{crypto::UncheckedInto, sr25519};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{FixedPointNumber, FixedU128, Perbill};

use crate::chain_spec::{
	evm_genesis, get_account_id_from_seed, get_authority_keys_from_seed, Extensions, TELEMETRY_URL,
};

pub type ChainSpec = sc_service::GenericChainSpec<mandala_runtime::GenesisConfig, Extensions>;

fn mandala_session_keys(grandpa: GrandpaId, babe: BabeId) -> mandala_runtime::SessionKeys {
	mandala_runtime::SessionKeys { grandpa, babe }
}

/// Development testnet config (single validator Alice)
pub fn development_testnet_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = mandala_runtime::WASM_BINARY.unwrap_or_default();

	Ok(ChainSpec::from_genesis(
		"Development",
		"dev",
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
				],
				true,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Default::default(),
	))
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

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
				false,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Default::default(),
	))
}

pub fn latest_mandala_testnet_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = mandala_runtime::WASM_BINARY.ok_or("Mandala runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Acala Mandala TC6",
		"mandala5",
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
				false,
			)
		},
		vec![
			// "/dns/testnet-bootnode-1.acala.laminar.one/tcp/30333/p2p/12D3KooWAFUNUowRqCV4c5so58Q8iGpypVf3L5ak91WrHf7rPuKz"
			// 	.parse()
			// 	.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("mandala6"),
		Some(properties),
		Default::default(),
	))
}

pub fn mandala_testnet_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../../resources/mandala-dist.json")[..])
}

fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> mandala_runtime::GenesisConfig {
	use mandala_runtime::{
		get_all_module_accounts, AcalaOracleConfig, AirDropConfig, BabeConfig, Balance, BalancesConfig,
		BandOracleConfig, CdpEngineConfig, CdpTreasuryConfig, ContractsConfig, CurrencyId, DexConfig, EVMConfig,
		EnabledTradingPairs, GeneralCouncilMembershipConfig, GrandpaConfig, HomaCouncilMembershipConfig,
		HonzonCouncilMembershipConfig, IndicesConfig, NativeTokenExistentialDeposit, OperatorMembershipAcalaConfig,
		OperatorMembershipBandConfig, OrmlNFTConfig, RenVmBridgeConfig, SessionConfig, StakerStatus, StakingConfig,
		StakingPoolConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokenSymbol, TokensConfig,
		TradingPair, VestingConfig, DOLLARS,
	};
	#[cfg(feature = "std")]
	use sp_std::collections::btree_map::BTreeMap;

	let existential_deposit = NativeTokenExistentialDeposit::get();

	const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
	const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

	let (evm_genesis_accounts, network_contract_index) = evm_genesis();

	// merge duplicated
	let balances = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), INITIAL_STAKING + DOLLARS)) // bit more for fee
		.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
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
		frame_system: Some(SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig { balances }),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), mandala_session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_STAKING, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance2: Some(HonzonCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance3: Some(Default::default()),
		pallet_membership_Instance3: Some(HomaCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance4: Some(Default::default()),
		pallet_membership_Instance4: Some(TechnicalCommitteeMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_membership_Instance5: Some(OperatorMembershipAcalaConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_membership_Instance6: Some(OperatorMembershipBandConfig {
			members: vec![root_key],
			phantom: Default::default(),
		}),
		pallet_treasury: Some(Default::default()),
		pallet_contracts: Some(ContractsConfig {
			current_schedule: pallet_contracts::Schedule {
				enable_println, // this should only be enabled on development chains
				..Default::default()
			},
		}),
		orml_tokens: Some(TokensConfig {
			endowed_accounts: endowed_accounts
				.iter()
				.flat_map(|x| {
					vec![
						(x.clone(), CurrencyId::Token(TokenSymbol::AUSD), INITIAL_BALANCE),
						(x.clone(), CurrencyId::Token(TokenSymbol::DOT), INITIAL_BALANCE),
						(x.clone(), CurrencyId::Token(TokenSymbol::XBTC), INITIAL_BALANCE),
					]
				})
				.collect(),
		}),
		orml_vesting: Some(VestingConfig { vesting: vec![] }),
		module_cdp_treasury: Some(CdpTreasuryConfig {
			collateral_auction_maximum_size: vec![
				(CurrencyId::Token(TokenSymbol::DOT), DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::Token(TokenSymbol::XBTC), DOLLARS),
				(CurrencyId::Token(TokenSymbol::RENBTC), DOLLARS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::Token(TokenSymbol::DOT),
					Some(FixedU128::zero()),                             // stability fee for this collateral
					Some(FixedU128::saturating_from_rational(150, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(10, 100)),  // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(150, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                                // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::Token(TokenSymbol::XBTC),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(150, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::Token(TokenSymbol::LDOT),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(180, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::Token(TokenSymbol::RENBTC),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(150, 100)),
					10_000_000 * DOLLARS,
				),
			],
			global_stability_fee: FixedU128::saturating_from_rational(618_850_393, 100_000_000_000_000_000_u128), /* 5% APR */
		}),
		module_airdrop: Some(AirDropConfig {
			airdrop_accounts: vec![],
		}),
		orml_oracle_Instance1: Some(AcalaOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		orml_oracle_Instance2: Some(BandOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		module_evm: Some(EVMConfig {
			accounts: evm_genesis_accounts,
			network_contract_index,
		}),
		module_staking_pool: Some(StakingPoolConfig {
			staking_pool_params: module_staking_pool::Params {
				target_max_free_unbonded_ratio: FixedU128::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: FixedU128::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: FixedU128::saturating_from_rational(2, 100),
				unbonding_to_free_adjustment: FixedU128::saturating_from_rational(1, 1000),
				base_fee_rate: FixedU128::saturating_from_rational(2, 100),
			},
		}),
		module_dex: Some(DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(
						TradingPair::new(
							CurrencyId::Token(TokenSymbol::AUSD),
							CurrencyId::Token(TokenSymbol::DOT),
						),
						(1_000_000u128, 2_000_000u128),
					),
					(
						TradingPair::new(
							CurrencyId::Token(TokenSymbol::AUSD),
							CurrencyId::Token(TokenSymbol::XBTC),
						),
						(1_000_000u128, 2_000_000u128),
					),
					(
						TradingPair::new(
							CurrencyId::Token(TokenSymbol::AUSD),
							CurrencyId::Token(TokenSymbol::ACA),
						),
						(1_000_000u128, 2_000_000u128),
					),
				],
			)],
		}),
		ecosystem_renvm_bridge: Some(RenVmBridgeConfig {
			ren_vm_public_key: hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"],
		}),
		orml_nft: Some(OrmlNFTConfig { tokens: vec![] }),
	}
}

fn mandala_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> mandala_runtime::GenesisConfig {
	use mandala_runtime::{
		get_all_module_accounts, AcalaOracleConfig, AirDropConfig, AirDropCurrencyId, BabeConfig, Balance,
		BalancesConfig, BandOracleConfig, CdpEngineConfig, CdpTreasuryConfig, ContractsConfig, CurrencyId, DexConfig,
		EVMConfig, EnabledTradingPairs, GeneralCouncilMembershipConfig, GrandpaConfig, HomaCouncilMembershipConfig,
		HonzonCouncilMembershipConfig, IndicesConfig, NativeTokenExistentialDeposit, OperatorMembershipAcalaConfig,
		OperatorMembershipBandConfig, OrmlNFTConfig, RenVmBridgeConfig, SessionConfig, StakerStatus, StakingConfig,
		StakingPoolConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokenSymbol, TokensConfig,
		VestingConfig, CENTS, DOLLARS,
	};
	#[cfg(feature = "std")]
	use sp_std::collections::btree_map::BTreeMap;

	let existential_deposit = NativeTokenExistentialDeposit::get();

	const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
	const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

	let (evm_genesis_accounts, network_contract_index) = evm_genesis();

	let balances = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), INITIAL_STAKING + DOLLARS)) // bit more for fee
		.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
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
		frame_system: Some(SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig { balances }),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), mandala_session_keys(x.2.clone(), x.3.clone())))
				.collect::<Vec<_>>(),
		}),
		pallet_staking: Some(StakingConfig {
			validator_count: 5,
			minimum_validator_count: 1,
			stakers: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.1.clone(), INITIAL_STAKING, StakerStatus::Validator))
				.collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			..Default::default()
		}),
		pallet_sudo: Some(SudoConfig { key: root_key.clone() }),
		pallet_babe: Some(BabeConfig { authorities: vec![] }),
		pallet_grandpa: Some(GrandpaConfig { authorities: vec![] }),
		pallet_collective_Instance1: Some(Default::default()),
		pallet_membership_Instance1: Some(GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance2: Some(Default::default()),
		pallet_membership_Instance2: Some(HonzonCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance3: Some(Default::default()),
		pallet_membership_Instance3: Some(HomaCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_collective_Instance4: Some(Default::default()),
		pallet_membership_Instance4: Some(TechnicalCommitteeMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		}),
		pallet_membership_Instance5: Some(OperatorMembershipAcalaConfig {
			members: endowed_accounts.clone(),
			phantom: Default::default(),
		}),
		pallet_membership_Instance6: Some(OperatorMembershipBandConfig {
			members: endowed_accounts,
			phantom: Default::default(),
		}),
		pallet_treasury: Some(Default::default()),
		pallet_contracts: Some(ContractsConfig {
			current_schedule: pallet_contracts::Schedule {
				enable_println, // this should only be enabled on development chains
				..Default::default()
			},
		}),
		orml_tokens: Some(TokensConfig {
			endowed_accounts: vec![
				(root_key.clone(), CurrencyId::Token(TokenSymbol::DOT), INITIAL_BALANCE),
				(root_key, CurrencyId::Token(TokenSymbol::XBTC), INITIAL_BALANCE),
			],
		}),
		orml_vesting: Some(VestingConfig { vesting: vec![] }),
		module_cdp_treasury: Some(CdpTreasuryConfig {
			collateral_auction_maximum_size: vec![
				(CurrencyId::Token(TokenSymbol::DOT), DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::Token(TokenSymbol::XBTC), 5 * CENTS),
				(CurrencyId::Token(TokenSymbol::RENBTC), 5 * CENTS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::Token(TokenSymbol::DOT),
					Some(FixedU128::zero()),                             // stability fee for this collateral
					Some(FixedU128::saturating_from_rational(105, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(3, 100)),   // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(110, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                                // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::Token(TokenSymbol::XBTC),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(110, 100)),
					Some(FixedU128::saturating_from_rational(4, 100)),
					Some(FixedU128::saturating_from_rational(115, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::Token(TokenSymbol::LDOT),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(120, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(130, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::Token(TokenSymbol::RENBTC),
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(110, 100)),
					Some(FixedU128::saturating_from_rational(4, 100)),
					Some(FixedU128::saturating_from_rational(115, 100)),
					10_000_000 * DOLLARS,
				),
			],
			global_stability_fee: FixedU128::saturating_from_rational(618_850_393, 100_000_000_000_000_000_u128), /* 5% APR */
		}),
		module_airdrop: Some(AirDropConfig {
			airdrop_accounts: {
				let aca_airdrop_accounts_json =
					&include_bytes!("../../../../../resources/mandala-airdrop-ACA.json")[..];
				let aca_airdrop_accounts: Vec<(AccountId, Balance)> =
					serde_json::from_slice(aca_airdrop_accounts_json).unwrap();
				let kar_airdrop_accounts_json =
					&include_bytes!("../../../../../resources/mandala-airdrop-KAR.json")[..];
				let kar_airdrop_accounts: Vec<(AccountId, Balance)> =
					serde_json::from_slice(kar_airdrop_accounts_json).unwrap();

				aca_airdrop_accounts
					.iter()
					.map(|(account_id, aca_amount)| (account_id.clone(), AirDropCurrencyId::ACA, *aca_amount))
					.chain(
						kar_airdrop_accounts
							.iter()
							.map(|(account_id, kar_amount)| (account_id.clone(), AirDropCurrencyId::KAR, *kar_amount)),
					)
					.collect::<Vec<_>>()
			},
		}),
		orml_oracle_Instance1: Some(AcalaOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		orml_oracle_Instance2: Some(BandOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		module_evm: Some(EVMConfig {
			accounts: evm_genesis_accounts,
			network_contract_index,
		}),
		module_staking_pool: Some(StakingPoolConfig {
			staking_pool_params: module_staking_pool::Params {
				target_max_free_unbonded_ratio: FixedU128::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: FixedU128::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: FixedU128::saturating_from_rational(2, 100),
				unbonding_to_free_adjustment: FixedU128::saturating_from_rational(1, 1000),
				base_fee_rate: FixedU128::saturating_from_rational(2, 100),
			},
		}),
		module_dex: Some(DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		}),
		ecosystem_renvm_bridge: Some(RenVmBridgeConfig {
			ren_vm_public_key: hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"],
		}),
		orml_nft: Some(OrmlNFTConfig {
			tokens: {
				let nft_airdrop_json = &include_bytes!("../../../../../resources/mandala-airdrop-NFT.json")[..];
				let nft_airdrop: Vec<(
					AccountId,
					Vec<u8>,
					module_nft::ClassData,
					Vec<(Vec<u8>, module_nft::TokenData, Vec<AccountId>)>,
				)> = serde_json::from_slice(nft_airdrop_json).unwrap();

				let mut tokens = vec![];
				for (class_owner, class_meta, class_data, nfts) in nft_airdrop {
					let mut tokens_of_class = vec![];
					for (token_meta, token_data, token_owners) in nfts {
						token_owners.iter().for_each(|account_id| {
							tokens_of_class.push((account_id.clone(), token_meta.clone(), token_data.clone()));
						});
					}

					tokens.push((
						class_owner.clone(),
						class_meta.clone(),
						class_data.clone(),
						tokens_of_class,
					));
				}

				tokens
			},
		}),
	}
}

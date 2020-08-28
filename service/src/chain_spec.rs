//! Acala chain configurations.

use acala_primitives::{AccountId, AccountPublic};
use hex_literal::hex;
use sc_chain_spec::{ChainSpecExtension, ChainType};
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use serde_json::map::Map;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{traits::IdentifyAccount, FixedPointNumber, FixedU128, Perbill};

// The URL for the telemetry server.
const TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<acala_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<acala_primitives::Block>,
}

/// The `ChainSpec parametrised for dev/mandala runtime`.
pub type DevChainSpec = sc_service::GenericChainSpec<dev_runtime::GenesisConfig, Extensions>;

fn dev_session_keys(grandpa: GrandpaId, babe: BabeId) -> dev_runtime::SessionKeys {
	dev_runtime::SessionKeys { grandpa, babe }
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Aura authority key.
pub fn get_authority_keys_from_seed(seed: &str) -> (AccountId, AccountId, GrandpaId, BabeId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<BabeId>(seed),
	)
}

/// Development testnet config (single validator Alice)
pub fn development_testnet_config() -> Result<DevChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = dev_runtime::WASM_BINARY.ok_or("Dev runtime wasm binary not available")?;

	Ok(DevChainSpec::from_genesis(
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
pub fn local_testnet_config() -> Result<DevChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = dev_runtime::WASM_BINARY.ok_or("Dev runtime wasm binary not available")?;

	Ok(DevChainSpec::from_genesis(
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

/// Latest Mandala testnet config
pub fn latest_mandala_testnet_config() -> Result<DevChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "ACA".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = dev_runtime::WASM_BINARY.ok_or("Dev runtime wasm binary not available")?;

	Ok(DevChainSpec::from_genesis(
		"Acala Mandala TC4",
		"mandala4",
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
			"/dns/testnet-bootnode-1.acala.laminar.one/tcp/30333/p2p/12D3KooWAFUNUowRqCV4c5so58Q8iGpypVf3L5ak91WrHf7rPuKz"
				.parse()
				.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("mandala4"),
		Some(properties),
		Default::default(),
	))
}

/// Mandala testnet generator
pub fn mandala_testnet_config() -> Result<DevChainSpec, String> {
	DevChainSpec::from_json_bytes(&include_bytes!("../../resources/mandala-dist.json")[..])
}

fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> dev_runtime::GenesisConfig {
	use dev_runtime::{
		get_all_module_accounts, AcalaOracleConfig, AirDropConfig, BabeConfig, BalancesConfig, BandOracleConfig,
		CdpEngineConfig, CdpTreasuryConfig, ContractsConfig, CurrencyId, DexConfig, GeneralCouncilMembershipConfig,
		GrandpaConfig, HomaCouncilMembershipConfig, HonzonCouncilMembershipConfig, IndicesConfig, NewAccountDeposit,
		OperatorMembershipAcalaConfig, OperatorMembershipBandConfig, PolkadotBridgeConfig, SessionConfig, StakerStatus,
		StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig,
		DOLLARS,
	};

	let new_account_deposit = NewAccountDeposit::get();

	const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
	const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

	dev_runtime::GenesisConfig {
		frame_system: Some(SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig {
			balances: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), INITIAL_STAKING + DOLLARS)) // bit more for fee
				.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
				.chain(
					get_all_module_accounts()
						.iter()
						.map(|x| (x.clone(), new_account_deposit)),
				)
				.collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), dev_session_keys(x.2.clone(), x.3.clone())))
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
						(x.clone(), CurrencyId::DOT, INITIAL_BALANCE),
						(x.clone(), CurrencyId::XBTC, INITIAL_BALANCE),
					]
				})
				.collect(),
		}),
		orml_vesting: Some(VestingConfig { vesting: vec![] }),
		module_cdp_treasury: Some(CdpTreasuryConfig {
			collateral_auction_maximum_size: vec![
				(CurrencyId::DOT, DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::XBTC, DOLLARS),
				(CurrencyId::RENBTC, DOLLARS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::DOT,
					Some(FixedU128::zero()),                             // stability fee for this collateral
					Some(FixedU128::saturating_from_rational(150, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(10, 100)),  // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(150, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                                // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::XBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(150, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::LDOT,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(180, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::RENBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(150, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(150, 100)),
					10_000_000 * DOLLARS,
				),
			],
			global_stability_fee: FixedU128::saturating_from_rational(618_850_393, 100_000_000_000_000_000_u128), /* 5% APR */
		}),
		module_dex: Some(DexConfig {
			liquidity_incentive_rate: vec![
				(CurrencyId::DOT, FixedU128::zero()),
				(CurrencyId::XBTC, FixedU128::zero()),
				(CurrencyId::LDOT, FixedU128::zero()),
				(CurrencyId::RENBTC, FixedU128::zero()),
			],
		}),
		module_polkadot_bridge: Some(PolkadotBridgeConfig {
			mock_reward_rate: FixedU128::saturating_from_rational(1, 100_000_000),
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
	}
}

fn mandala_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> dev_runtime::GenesisConfig {
	use dev_runtime::{
		get_all_module_accounts, AcalaOracleConfig, AirDropConfig, AirDropCurrencyId, BabeConfig, Balance,
		BalancesConfig, BandOracleConfig, CdpEngineConfig, CdpTreasuryConfig, ContractsConfig, CurrencyId, DexConfig,
		GeneralCouncilMembershipConfig, GrandpaConfig, HomaCouncilMembershipConfig, HonzonCouncilMembershipConfig,
		IndicesConfig, NewAccountDeposit, OperatorMembershipAcalaConfig, OperatorMembershipBandConfig,
		PolkadotBridgeConfig, SessionConfig, StakerStatus, StakingConfig, SudoConfig, SystemConfig,
		TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig, CENTS, DOLLARS,
	};

	let new_account_deposit = NewAccountDeposit::get();

	const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
	const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

	dev_runtime::GenesisConfig {
		frame_system: Some(SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_indices: Some(IndicesConfig { indices: vec![] }),
		pallet_balances: Some(BalancesConfig {
			balances: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), INITIAL_STAKING + DOLLARS)) // bit more for fee
				.chain(endowed_accounts.iter().cloned().map(|k| (k, INITIAL_BALANCE)))
				.chain(
					get_all_module_accounts()
						.iter()
						.map(|x| (x.clone(), new_account_deposit)),
				)
				.collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), dev_session_keys(x.2.clone(), x.3.clone())))
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
			members: endowed_accounts.clone(),
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
				(root_key.clone(), CurrencyId::DOT, INITIAL_BALANCE),
				(root_key, CurrencyId::XBTC, INITIAL_BALANCE),
			],
		}),
		orml_vesting: Some(VestingConfig { vesting: vec![] }),
		module_cdp_treasury: Some(CdpTreasuryConfig {
			collateral_auction_maximum_size: vec![
				(CurrencyId::DOT, DOLLARS), // (currency_id, max size of a collateral auction)
				(CurrencyId::XBTC, 5 * CENTS),
				(CurrencyId::RENBTC, 5 * CENTS),
			],
		}),
		module_cdp_engine: Some(CdpEngineConfig {
			collaterals_params: vec![
				(
					CurrencyId::DOT,
					Some(FixedU128::zero()),                             // stability fee for this collateral
					Some(FixedU128::saturating_from_rational(105, 100)), // liquidation ratio
					Some(FixedU128::saturating_from_rational(3, 100)),   // liquidation penalty rate
					Some(FixedU128::saturating_from_rational(110, 100)), // required liquidation ratio
					10_000_000 * DOLLARS,                                // maximum debit value in aUSD (cap)
				),
				(
					CurrencyId::XBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(110, 100)),
					Some(FixedU128::saturating_from_rational(4, 100)),
					Some(FixedU128::saturating_from_rational(115, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::LDOT,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(120, 100)),
					Some(FixedU128::saturating_from_rational(10, 100)),
					Some(FixedU128::saturating_from_rational(130, 100)),
					10_000_000 * DOLLARS,
				),
				(
					CurrencyId::RENBTC,
					Some(FixedU128::zero()),
					Some(FixedU128::saturating_from_rational(110, 100)),
					Some(FixedU128::saturating_from_rational(4, 100)),
					Some(FixedU128::saturating_from_rational(115, 100)),
					10_000_000 * DOLLARS,
				),
			],
			global_stability_fee: FixedU128::saturating_from_rational(618_850_393, 100_000_000_000_000_000_u128), /* 5% APR */
		}),
		module_dex: Some(DexConfig {
			liquidity_incentive_rate: vec![
				(
					CurrencyId::DOT,
					FixedU128::saturating_from_rational(4975, 10_000_000_000_000_u128),
				), // 4% APR
				(
					CurrencyId::XBTC,
					FixedU128::saturating_from_rational(4975, 10_000_000_000_000_u128),
				), // 4% APR
				(
					CurrencyId::LDOT,
					FixedU128::saturating_from_rational(4975, 10_000_000_000_000_u128),
				), // 4% APR
				(
					CurrencyId::RENBTC,
					FixedU128::saturating_from_rational(4975, 10_000_000_000_000_u128),
				), // 4% APR
			],
		}),
		module_polkadot_bridge: Some(PolkadotBridgeConfig {
			mock_reward_rate: FixedU128::saturating_from_rational(5, 10000), // 20% APR
		}),
		module_airdrop: Some(AirDropConfig {
			airdrop_accounts: {
				let airdrop_accounts_json = &include_bytes!("../../resources/mandala-airdrop-accounts.json")[..];
				let airdrop_accounts: Vec<(AccountId, AirDropCurrencyId, Balance)> =
					serde_json::from_slice(airdrop_accounts_json).unwrap();
				airdrop_accounts
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
	}
}

use acala_primitives::{AccountId, AirDropCurrencyId};
use hex_literal::hex;
use sc_chain_spec::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_core::crypto::UncheckedInto;
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{FixedPointNumber, FixedU128, Perbill};

use crate::chain_spec::{Extensions, TELEMETRY_URL};

pub type ChainSpec = sc_service::GenericChainSpec<karura_runtime::GenesisConfig, Extensions>;

fn karura_session_keys(grandpa: GrandpaId, babe: BabeId) -> karura_runtime::SessionKeys {
	karura_runtime::SessionKeys { grandpa, babe }
}

pub fn karura_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
}

pub fn latest_karura_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	properties.insert("tokenSymbol".into(), "KAR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	let wasm_binary = karura_runtime::WASM_BINARY.ok_or("Karura runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Acala Karura",
		"karura",
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
			karura_genesis(
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
			//TODO
			"/dns/testnet-bootnode-1.karura.laminar.one/tcp/30333/p2p/12D3KooWAFUNUowRqCV4c5so58Q8iGpypVf3L5ak91WrHf7rPuKz"
				.parse()
				.unwrap(),
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("karura"),
		Some(properties),
		Default::default(),
	))
}

fn karura_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	enable_println: bool,
) -> karura_runtime::GenesisConfig {
	use karura_runtime::{
		get_all_module_accounts, AcalaOracleConfig, BabeConfig, Balance, BalancesConfig, BandOracleConfig,
		CdpEngineConfig, CdpTreasuryConfig, ContractsConfig, CurrencyId, GeneralCouncilMembershipConfig, GrandpaConfig,
		HomaCouncilMembershipConfig, HonzonCouncilMembershipConfig, IndicesConfig, NewAccountDeposit,
		OperatorMembershipAcalaConfig, OperatorMembershipBandConfig, PolkadotBridgeConfig, SessionConfig, StakerStatus,
		StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig, TokenSymbol, TokensConfig,
		VestingConfig, CENTS, DOLLARS,
	};

	let new_account_deposit = NewAccountDeposit::get();
	let airdrop_accounts = {
		let airdrop_accounts_json = &include_bytes!("../../../resources/mandala-airdrop-accounts.json")[..];
		let airdrop_accounts: Vec<(AccountId, AirDropCurrencyId, Balance)> =
			serde_json::from_slice(airdrop_accounts_json).unwrap();
		airdrop_accounts
			.into_iter()
			.filter(|(_, currency_id, _)| *currency_id == AirDropCurrencyId::KAR)
			.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
			.collect::<Vec<_>>()
	};

	const INITIAL_BALANCE: u128 = 1_000_000 * DOLLARS;
	const INITIAL_STAKING: u128 = 100_000 * DOLLARS;

	karura_runtime::GenesisConfig {
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
				.chain(airdrop_accounts)
				.collect(),
		}),
		pallet_session: Some(SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), karura_session_keys(x.2.clone(), x.3.clone())))
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
		module_polkadot_bridge: Some(PolkadotBridgeConfig {
			mock_reward_rate: FixedU128::saturating_from_rational(5, 10000), // 20% APR
		}),
		orml_oracle_Instance1: Some(AcalaOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		orml_oracle_Instance2: Some(BandOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		}),
		pallet_evm: Some(Default::default()),
	}
}

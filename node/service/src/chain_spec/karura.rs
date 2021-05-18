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

use acala_primitives::{AccountId, TokenSymbol};
use hex_literal::hex;
use sc_chain_spec::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde_json::map::Map;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519};
use sp_runtime::traits::Zero;

use crate::chain_spec::{get_account_id_from_seed, get_karura_authority_keys_from_seed, Extensions, TELEMETRY_URL};

pub type ChainSpec = sc_service::GenericChainSpec<karura_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 2000;

pub fn karura_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/karura-dist.json")[..])
}

pub fn latest_karura_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	TokenSymbol::get_info().iter().for_each(|(symbol_name, decimals)| {
		token_symbol.push(symbol_name.to_string());
		token_decimals.push(*decimals);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());

	let wasm_binary = karura_runtime::WASM_BINARY.ok_or("Karura runtime wasm binary not available")?;

	Ok(ChainSpec::from_genesis(
		"Karura",
		"karura",
		ChainType::Live,
		move || {
			karura_genesis(
				wasm_binary,
				vec![
					(
						// qkFZUE2Dod2Y9LX8ZQzkvF5T2wE5hpBuPe9hT1gpP3drH1v
						hex!["6c47c55604029bd43ed443ddaad370d5f4c10fa439d22dddb8120a9615444b6b"].into(),
						hex!["36589a134ccdbeb45a3ac535cc2c8cd71ae45ffc3af86d4a020cc2e411a98875"].unchecked_into(),
					),
					(
						// pSCWXtDyPZsyfTQNbVkmubVRGyoSi9N2a6AxpWHWFsxLjXs
						hex!["3246d9cb076cd554f250fc03bf70988cbaa9cbb2c4b1b8e015dd97fd19405d43"].into(),
						hex!["3266d0febeacc5d111c9df7f2ced2f533e7732dda46b2b84f104be5d6e395b76"].unchecked_into(),
					),
					(
						// qZhHE2FJGGAJtvu9f21PPFVDxvcnm65ebezZBsAJjGFa4kn
						hex!["643aa70071341b904e6e5b4e41d6dfc02b4cfcdc4c9c7a66f41fc0e59c07e24c"].into(),
						hex!["60fdcbd860869ee9b1230731b82604e8cf63c6c66e69277b59e337f1f25af225"].unchecked_into(),
					),
				],
				// sWcq8FAQXPdXGSaxSTBKS614hCB8YutkVWWacBKG1GbGS23
				hex!["ba5a672d05b5db2ff433ee3dc24cf021e301bc9d44232046ce7bd45a9360fa50"].into(),
			)
		},
		vec![
			// no bootnode yet
		],
		TelemetryEndpoints::new(vec![(TELEMETRY_URL.into(), 0)]).ok(),
		Some("karura"),
		Some(properties),
		Extensions {
			relay_chain: "kusama".into(),
			para_id: PARA_ID,
		},
	))
}

pub fn karura_dev_config() -> Result<ChainSpec, String> {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	TokenSymbol::get_info().iter().for_each(|(symbol_name, decimals)| {
		token_symbol.push(symbol_name.to_string());
		token_decimals.push(*decimals);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());

	let wasm_binary = karura_runtime::WASM_BINARY.unwrap_or_default();

	Ok(ChainSpec::from_genesis(
		"Acala Karura Dev",
		"karura-dev",
		ChainType::Development,
		move || {
			karura_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![get_karura_authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
		},
	))
}

fn karura_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AuraId)>,
	root_key: AccountId,
) -> karura_runtime::GenesisConfig {
	use karura_runtime::{
		dollar, get_all_module_accounts, AcalaOracleConfig, Balance, BalancesConfig, BlockNumber, CdpEngineConfig,
		CdpTreasuryConfig, CollatorSelectionConfig, DexConfig, GeneralCouncilMembershipConfig,
		HomaCouncilMembershipConfig, HonzonCouncilMembershipConfig, NativeTokenExistentialDeposit,
		OperatorMembershipAcalaConfig, OrmlNFTConfig, ParachainInfoConfig, SessionConfig, SessionKeys, SudoConfig,
		SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, TreasuryPalletId, VestingConfig, KAR,
	};
	use sp_runtime::traits::AccountIdConversion;
	use sp_std::collections::btree_map::BTreeMap;

	let existential_deposit = NativeTokenExistentialDeposit::get();
	let mut total_allocated: Balance = Zero::zero();

	let airdrop_accounts_json = &include_bytes!("../../../../resources/mandala-airdrop-KAR.json")[..];
	let airdrop_accounts: Vec<(AccountId, Balance)> = serde_json::from_slice(airdrop_accounts_json).unwrap();
	let other_allocation_json = &include_bytes!("../../../../resources/karura-allocation-KAR.json")[..];
	let other_allocation: Vec<(AccountId, Balance)> = serde_json::from_slice(other_allocation_json).unwrap();

	let initial_allocation = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), existential_deposit))
		.chain(airdrop_accounts)
		.chain(other_allocation)
		// Put all the remaining to treasury for now. Remove this later.
		.chain(vec![(TreasuryPalletId::get().into_account(), 96322899587000000000)])
		.chain(
			get_all_module_accounts()
				.iter()
				.map(|x| (x.clone(), existential_deposit)), // add ED for module accounts
		)
		.fold(
			BTreeMap::<AccountId, Balance>::new(),
			|mut acc, (account_id, amount)| {
				// merge duplicated accounts
				if let Some(balance) = acc.get_mut(&account_id) {
					*balance = balance
						.checked_add(amount)
						.expect("balance cannot overflow when building genesis");
				} else {
					acc.insert(account_id.clone(), amount);
				}

				total_allocated = total_allocated.saturating_add(amount);
				acc
			},
		)
		.into_iter()
		.collect::<Vec<(AccountId, Balance)>>();

	// check total allocated
	assert_eq!(
		total_allocated,
		100_000_000 * dollar(KAR), // 100 million KAR
		"total allocation must be equal to 100 million KAR"
	);

	let vesting_list_json = &include_bytes!("../../../../resources/karura-vesting-KAR.json")[..];
	let vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)> =
		serde_json::from_slice(vesting_list_json).unwrap();

	// ensure no duplicates exist.
	let unique_vesting_accounts = vesting_list
		.iter()
		.map(|(x, _, _, _, _)| x)
		.cloned()
		.collect::<std::collections::BTreeSet<_>>();
	assert!(
		unique_vesting_accounts.len() == vesting_list.len(),
		"duplicate vesting accounts in genesis."
	);

	karura_runtime::GenesisConfig {
		frame_system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: BalancesConfig {
			balances: initial_allocation,
		},
		pallet_sudo: SudoConfig { key: root_key },
		pallet_collective_Instance1: Default::default(),
		pallet_membership_Instance1: GeneralCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: Default::default(),
		pallet_membership_Instance2: HonzonCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_collective_Instance3: Default::default(),
		pallet_membership_Instance3: HomaCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_collective_Instance4: Default::default(),
		pallet_membership_Instance4: TechnicalCommitteeMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_membership_Instance5: OperatorMembershipAcalaConfig {
			members: vec![],
			phantom: Default::default(),
		},
		pallet_treasury: Default::default(),
		orml_tokens: TokensConfig {
			endowed_accounts: vec![],
		},
		orml_vesting: VestingConfig { vesting: vesting_list },
		module_cdp_treasury: CdpTreasuryConfig {
			expected_collateral_auction_size: vec![],
		},
		module_cdp_engine: CdpEngineConfig {
			collaterals_params: vec![],
			global_interest_rate_per_sec: Default::default(),
		},
		orml_oracle_Instance1: AcalaOracleConfig {
			members: Default::default(), // initialized by OperatorMembership
			phantom: Default::default(),
		},
		module_evm: Default::default(),
		module_dex: DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: vec![],
			initial_added_liquidity_pools: vec![],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		module_collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: Zero::zero(),
			..Default::default()
		},
		pallet_session: SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),          // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		pallet_aura: Default::default(),
		cumulus_pallet_aura_ext: Default::default(),
	}
}

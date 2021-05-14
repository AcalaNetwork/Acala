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
use sp_core::crypto::UncheckedInto;
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::Zero;

use crate::chain_spec::{Extensions, TELEMETRY_URL};

pub type ChainSpec = sc_service::GenericChainSpec<karura_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 1000;

pub fn karura_config() -> Result<ChainSpec, String> {
	Err("Not available".into())
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
		Extensions {
			relay_chain: "rococo".into(),
			para_id: PARA_ID,
		},
	))
}

fn karura_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, AuraId)>,
	root_key: AccountId,
) -> karura_runtime::GenesisConfig {
	use karura_runtime::{
		dollar, get_all_module_accounts, AcalaOracleConfig, Balance, BalancesConfig, BlockNumber, CdpEngineConfig,
		CdpTreasuryConfig, CollatorSelectionConfig, DexConfig, EnabledTradingPairs, GeneralCouncilMembershipConfig,
		HomaCouncilMembershipConfig, HonzonCouncilMembershipConfig, NativeTokenExistentialDeposit,
		OperatorMembershipAcalaConfig, OrmlNFTConfig, ParachainInfoConfig, SessionConfig, SessionKeys, SudoConfig,
		SystemConfig, TechnicalCommitteeMembershipConfig, TokensConfig, VestingConfig, KAR,
	};
	#[cfg(feature = "std")]
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
	assert!(
		total_allocated == 100_000_000 * dollar(KAR), // 100 million KAR
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
		pallet_sudo: SudoConfig { key: root_key.clone() },
		pallet_collective_Instance1: Default::default(),
		pallet_membership_Instance1: GeneralCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		pallet_collective_Instance2: Default::default(),
		pallet_membership_Instance2: HonzonCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		pallet_collective_Instance3: Default::default(),
		pallet_membership_Instance3: HomaCouncilMembershipConfig {
			members: vec![root_key.clone()],
			phantom: Default::default(),
		},
		pallet_collective_Instance4: Default::default(),
		pallet_membership_Instance4: TechnicalCommitteeMembershipConfig {
			members: vec![root_key],
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
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		module_collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _, _, _)| acc).collect(),
			candidacy_bond: Zero::zero(),
			..Default::default()
		},
		pallet_session: SessionConfig {
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
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		pallet_aura: Default::default(),
	}
}

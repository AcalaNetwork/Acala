// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

use acala_primitives::AccountId;
use sc_chain_spec::{ChainType, Properties};
use serde_json::map::Map;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::sr25519;
use sp_runtime::traits::Zero;

use crate::chain_spec::{get_account_id_from_seed, get_parachain_authority_keys_from_seed, Extensions};

use karura_runtime::{
	dollar, Balance, BalancesConfig, BlockNumber, CdpEngineConfig, CdpTreasuryConfig, CollatorSelectionConfig,
	DexConfig, FinancialCouncilMembershipConfig, GeneralCouncilMembershipConfig, HomaCouncilMembershipConfig,
	OperatorMembershipAcalaConfig, OrmlNFTConfig, ParachainInfoConfig, PolkadotXcmConfig, SS58Prefix, SessionConfig,
	SessionDuration, SessionKeys, SessionManagerConfig, SudoConfig, SystemConfig, TechnicalCommitteeMembershipConfig,
	TokensConfig, VestingConfig, BNC, KAR, KSM, KUSD, LKSM, PHA, VSKSM,
};
use runtime_common::TokenInfo;

pub type ChainSpec = sc_service::GenericChainSpec<karura_runtime::GenesisConfig, Extensions>;

pub const PARA_ID: u32 = 2000;

pub fn karura_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../../../../resources/karura-dist.json")[..])
}

fn karura_properties() -> Properties {
	let mut properties = Map::new();
	let mut token_symbol: Vec<String> = vec![];
	let mut token_decimals: Vec<u32> = vec![];
	[KAR, KUSD, KSM, LKSM, BNC, VSKSM, PHA].iter().for_each(|token| {
		token_symbol.push(token.symbol().unwrap().to_string());
		token_decimals.push(token.decimals().unwrap() as u32);
	});
	properties.insert("tokenSymbol".into(), token_symbol.into());
	properties.insert("tokenDecimals".into(), token_decimals.into());
	properties.insert("ss58Format".into(), SS58Prefix::get().into());

	properties
}

pub fn karura_dev_config() -> Result<ChainSpec, String> {
	let wasm_binary = karura_runtime::WASM_BINARY.unwrap_or_default();

	Ok(ChainSpec::from_genesis(
		"Acala Karura Dev",
		"karura-dev",
		ChainType::Development,
		move || {
			karura_genesis(
				wasm_binary,
				// Initial PoA authorities
				vec![get_parachain_authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), 1000 * dollar(KAR)),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), 1000 * dollar(KAR)),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						1000 * dollar(KAR),
					),
				],
				vec![],
				vec![get_account_id_from_seed::<sr25519::Public>("Alice")],
			)
		},
		vec![],
		None,
		None,
		None,
		Some(karura_properties()),
		Extensions {
			relay_chain: "rococo-local".into(),
			para_id: PARA_ID,
			bad_blocks: None,
		},
	))
}

fn karura_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AuraId)>,
	root_key: AccountId,
	initial_allocation: Vec<(AccountId, Balance)>,
	vesting_list: Vec<(AccountId, BlockNumber, BlockNumber, u32, Balance)>,
	general_councils: Vec<AccountId>,
) -> karura_runtime::GenesisConfig {
	karura_runtime::GenesisConfig {
		system: SystemConfig {
			// Add Wasm runtime to storage.
			code: wasm_binary.to_vec(),
		},
		balances: BalancesConfig {
			balances: initial_allocation,
		},
		sudo: SudoConfig { key: Some(root_key) },
		general_council: Default::default(),
		general_council_membership: GeneralCouncilMembershipConfig {
			members: general_councils,
			phantom: Default::default(),
		},
		financial_council: Default::default(),
		financial_council_membership: FinancialCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		homa_council: Default::default(),
		homa_council_membership: HomaCouncilMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		technical_committee: Default::default(),
		technical_committee_membership: TechnicalCommitteeMembershipConfig {
			members: vec![],
			phantom: Default::default(),
		},
		operator_membership_acala: OperatorMembershipAcalaConfig {
			members: vec![],
			phantom: Default::default(),
		},
		democracy: Default::default(),
		treasury: Default::default(),
		tokens: TokensConfig { balances: vec![] },
		vesting: VestingConfig { vesting: vesting_list },
		cdp_treasury: CdpTreasuryConfig {
			expected_collateral_auction_size: vec![],
		},
		cdp_engine: CdpEngineConfig {
			collaterals_params: vec![],
			global_interest_rate_per_sec: Default::default(),
		},
		evm: Default::default(),
		dex: DexConfig {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: vec![],
			initial_added_liquidity_pools: vec![],
		},
		parachain_info: ParachainInfoConfig {
			parachain_id: PARA_ID.into(),
		},
		orml_nft: OrmlNFTConfig { tokens: vec![] },
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: Zero::zero(),
			..Default::default()
		},
		session: SessionConfig {
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

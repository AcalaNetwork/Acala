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

//! A list of the different weight modules for our runtime.
#![allow(clippy::unnecessary_cast)]

pub mod module_aggregated_dex;
pub mod module_asset_registry;
pub mod module_auction_manager;
pub mod module_cdp_engine;
pub mod module_cdp_treasury;
pub mod module_collator_selection;
pub mod module_currencies;
pub mod module_dex;
pub mod module_dex_oracle;
pub mod module_emergency_shutdown;
pub mod module_evm;
pub mod module_evm_accounts;
pub mod module_homa;
pub mod module_honzon;
pub mod module_incentives;
pub mod module_nft;
pub mod module_prices;
pub mod module_session_manager;
pub mod module_transaction_pause;
pub mod module_transaction_payment;

pub mod orml_auction;
pub mod orml_authority;
pub mod orml_oracle;
pub mod orml_tokens;
pub mod orml_vesting;

pub mod nutsfinance_stable_asset;

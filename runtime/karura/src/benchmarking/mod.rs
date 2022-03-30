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

#![cfg(feature = "runtime-benchmarks")]

pub mod utils;

// module benchmarking
pub mod asset_registry {
	include!("../../../mandala/src/benchmarking/asset_registry.rs");
}
pub mod auction_manager {
	include!("../../../mandala/src/benchmarking/auction_manager.rs");
}
pub mod cdp_engine {
	include!("../../../mandala/src/benchmarking/cdp_engine.rs");
}
pub mod cdp_treasury {
	include!("../../../mandala/src/benchmarking/cdp_treasury.rs");
}
pub mod collator_selection {
	include!("../../../mandala/src/benchmarking/collator_selection.rs");
}
pub mod currencies {
	include!("../../../mandala/src/benchmarking/currencies.rs");
}
pub mod dex {
	include!("../../../mandala/src/benchmarking/dex.rs");
}
pub mod dex_oracle {
	include!("../../../mandala/src/benchmarking/dex_oracle.rs");
}
pub mod emergency_shutdown {
	include!("../../../mandala/src/benchmarking/emergency_shutdown.rs");
}
pub mod evm {
	include!("../../../mandala/src/benchmarking/evm.rs");
}
pub mod evm_accounts {
	include!("../../../mandala/src/benchmarking/evm_accounts.rs");
}
pub mod homa {
	include!("../../../mandala/src/benchmarking/homa.rs");
}
pub mod honzon {
	include!("../../../mandala/src/benchmarking/honzon.rs");
}
pub mod incentives {
	include!("../../../mandala/src/benchmarking/incentives.rs");
}
pub mod prices {
	include!("../../../mandala/src/benchmarking/prices.rs");
}
pub mod transaction_pause {
	include!("../../../mandala/src/benchmarking/transaction_pause.rs");
}
pub mod transaction_payment {
	include!("../../../mandala/src/benchmarking/transaction_payment.rs");
}
pub mod session_manager {
	include!("../../../mandala/src/benchmarking/session_manager.rs");
}
pub mod nutsfinance_stable_asset {
	include!("../../../mandala/src/benchmarking/nutsfinance_stable_asset.rs");
}

// orml benchmarking
pub mod auction {
	include!("../../../mandala/src/benchmarking/auction.rs");
}
pub mod authority {
	include!("../../../mandala/src/benchmarking/authority.rs");
}
pub mod oracle {
	include!("../../../mandala/src/benchmarking/oracle.rs");
}
pub mod tokens {
	include!("../../../mandala/src/benchmarking/tokens.rs");
}
pub mod vesting {
	include!("../../../mandala/src/benchmarking/vesting.rs");
}
pub mod honzon_bridge;

pub fn get_vesting_account() -> super::AccountId {
	super::KaruraFoundationAccounts::get()[0].clone()
}

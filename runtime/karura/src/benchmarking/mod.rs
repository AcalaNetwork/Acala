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

#![cfg(feature = "runtime-benchmarks")]

pub mod utils;

// module benchmarking
pub mod auction_manager {
	benchmarks::auction_manager_benchmarks!();
}
pub mod cdp_engine {
	benchmarks::cdp_engine_benchmarks!();
}
pub mod cdp_treasury {
	benchmarks::cdp_treasury_benchmarks!();
}
pub mod collator_selection {
	benchmarks::collator_selection_benchmarks!();
}
pub mod dex {
	benchmarks::dex_benchmarks!();
}
pub mod emergency_shutdown {
	benchmarks::emergency_shutdown_benchmarks!();
}
pub mod evm {
	benchmarks::evm_benchmarks!();
}
pub mod evm_accounts {
	benchmarks::evm_accounts_benchmarks!();
}
pub mod honzon {
	benchmarks::honzon_benchmarks!();
}
pub mod incentives {
	benchmarks::incentives_benchmarks!();
}
pub mod prices {
	benchmarks::prices_benchmarks!();
}
pub mod transaction_payment {
	benchmarks::transaction_payment_benchmarks!();
}
pub mod session_manager {
	benchmarks::session_manager_benchmarks!();
}

// orml benchmarking
pub mod auction {
	benchmarks::auction_benchmarks!();
}
pub mod authority {
	benchmarks::authority_benchmarks!();
}
pub mod currencies {
	benchmarks::currencies_benchmarks!();
}
pub mod oracle {
	benchmarks::oracle_benchmarks!();
}
pub mod tokens {
	benchmarks::tokens_benchmarks!();
}
pub mod vesting {
	benchmarks::vesting_benchmarks!();
}

pub fn get_treasury_account() -> super::AccountId {
	super::KaruraFoundationAccounts::get()[0].clone()
}

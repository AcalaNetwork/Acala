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

#![cfg_attr(not(feature = "std"), no_std)]

mod auction;
mod auction_manager;
mod authority;
mod cdp_engine;
mod cdp_treasury;
mod collator_selection;
mod currencies;
mod dex;
mod emergency_shutdown;
mod evm;
mod evm_accounts;
mod honzon;
mod incentives;
mod nominees_election;
mod oracle;
mod prices;
mod tokens;
mod transaction_payment;
mod vesting;

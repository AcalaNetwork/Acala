// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

#![cfg(test)]

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod setup;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod authority;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod dex;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod evm;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod honzon;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod nft;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod prices;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod proxy;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod runtime;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod session_manager;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod stable_asset;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod treasury;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod vesting;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod weights;

#[cfg(any(
	feature = "with-mandala-runtime",
	feature = "with-karura-runtime",
	feature = "with-acala-runtime"
))]
mod payment;

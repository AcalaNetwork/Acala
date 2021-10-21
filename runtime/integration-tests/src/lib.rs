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

#![cfg(test)]

#[cfg(any(feature = "with-mandala-runtime", feature = "with-karura-runtime",))]
mod integration_tests;

#[cfg(any(feature = "with-mandala-runtime", feature = "with-karura-runtime",))]
mod homa_lite_tests;

#[cfg(any(feature = "with-mandala-runtime", feature = "with-karura-runtime",))]
mod evm_tests;

#[cfg(any(feature = "with-mandala-runtime", feature = "with-karura-runtime",))]
mod weights_test;

#[cfg(feature = "with-karura-runtime")]
mod kusama_cross_chain_transfer;
#[cfg(feature = "with-karura-runtime")]
mod kusama_test_net;
#[cfg(feature = "with-karura-runtime")]
mod relay_chain_tests;

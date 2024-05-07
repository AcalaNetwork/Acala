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

#[doc(hidden)]
pub use orml_traits;
#[doc(hidden)]
pub use paste;

#[macro_export]
macro_rules! mock_handler {
	(
		$vis:vis struct $name:ident < $t:ty > ;
		$( $rest:tt )*
	) => {
		$crate::testing::paste::item! {
			thread_local! {
				pub static [<$name:snake:upper>]: std::cell::RefCell<Vec<$t>> = std::cell::RefCell::new(Vec::new());
			}

			$vis struct $name;

			impl $name {

				pub fn push(val: $t) {
					[<$name:snake:upper>].with(|v| v.borrow_mut().push(val));
				}

				pub fn clear() {
					[<$name:snake:upper>].with(|v| v.borrow_mut().clear());
				}

				pub fn get_all() {
					[<$name:snake:upper>].with(|v| v.borrow().clone());
				}

				pub fn assert_eq(expected: Vec<$t>) {
					[<$name:snake:upper>].with(|v| {
						assert_eq!(*v.borrow(), expected);
					});
				}

				pub fn assert_eq_and_clear(expected: Vec<$t>) {
					Self::assert_eq(expected);
					Self::clear();
				}

				pub fn assert_empty() {
					Self::assert_eq(Vec::new());
				}
			}

			impl $crate::testing::orml_traits::Happened<$t> for $name {
				fn happened(val: &$t) {
					Self::push(val.clone());
				}
			}

			impl $crate::testing::orml_traits::Handler<$t> for $name {
				fn handle(val: &$t) -> DispatchResult {
					Self::push(val.clone());
					Ok(())
				}
			}
		}

		$crate::mock_handler!( $( $rest )* );
	};
	() => {};
}

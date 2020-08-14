//! Common runtime code for Acala and Karura.

#![cfg_attr(not(feature = "std"), no_std)]

pub use module_support::{ExchangeRate, Price, Rate, Ratio};
pub use orml_oracle::AuthorityId as OracleId;

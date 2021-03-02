//! A list of the different weight modules for our runtime.
#![allow(clippy::unnecessary_cast)]

pub mod module_auction_manager;
pub mod module_cdp_engine;
pub mod module_cdp_treasury;
pub mod module_currencies;
pub mod module_dex;
pub mod module_emergency_shutdown;
pub mod module_evm;
pub mod module_evm_accounts;
pub mod module_homa;
pub mod module_honzon;
pub mod module_incentives;
pub mod module_nft;
pub mod module_prices;
pub mod module_transaction_payment;

pub mod orml_auction;
pub mod orml_authority;
pub mod orml_gradually_update;
pub mod orml_oracle;
pub mod orml_rewards;
pub mod orml_tokens;
pub mod orml_vesting;

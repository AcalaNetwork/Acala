#![cfg(feature = "runtime-benchmarks")]

// module benchmarking
pub mod auction_manager;
pub mod cdp_engine;
pub mod cdp_treasury;
pub mod dex;
pub mod emergency_shutdown;
pub mod evm;
pub mod evm_accounts;
pub mod homa;
pub mod honzon;
pub mod incentives;
pub mod prices;
pub mod transaction_payment;

// orml benchmarking
pub mod auction;
pub mod authority;
pub mod currencies;
pub mod gradually_update;
pub mod oracle;
pub mod rewards;
pub mod tokens;
pub mod utils;
pub mod vesting;

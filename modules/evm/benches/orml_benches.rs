use module_evm::bench_mock::{AllPalletsWithSystem, Block};
orml_bencher::run_benches!(AllPalletsWithSystem, Block);

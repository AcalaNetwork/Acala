#!/usr/bin/env bash

set -e

echo "*** Benchmark EVM"

cargo bench -p module-evm --features bench | (cd evm-bench && yarn analyze-benches ../runtime/common/src/gas_to_weight_ratio.rs)

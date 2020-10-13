# example for set acala/cdp-engine/max-iterations to 1000
#
# codec:
#   acala/cdp-engine/max-iterations/ = 0x6163616c612f6364702d656e67696e652f6d61782d697465726174696f6e732f,
#   acala/auction-manager/max-iterations/ = 0x6163616c612f61756374696f6e2d6d616e616765722f6d61782d697465726174696f6e732f,
# Litter-endian u32:
#   10 = 0x0a000000
#   100 = 0x64000000
#   1000 = 0xe803000000
#   10000 = 0x1027000000

curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "offchain_localStorageSet", "params":["PERSISTENT", "0x6163616c612f6364702d656e67696e652f6d61782d697465726174696f6e732f", "0xe803000000"]}' http://localhost:9933

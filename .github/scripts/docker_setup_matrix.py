import sys
import inspect

branch = sys.argv[1] if len(sys.argv) > 1 else ''

if branch.__contains__("release-"):
    chain, version = inspect.get_chain_and_version(branch)
    print("::set-output name=matrix::{\"network\": [\"" + chain + "\"]}")
else:
    print("::set-output name=matrix::{\"network\": [\"mandala\", \"karura\", \"acala\"]}")

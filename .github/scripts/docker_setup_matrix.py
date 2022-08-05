import os
import inspect

branch = os.getenv("GITHUB_REF")

if branch.__contains__("release-"):
    chain, version = inspect.get_chain_and_version(os.getenv("GITHUB_REF"))
    print("::set-output name=matrix::{\"network\": [\"" + chain + "\"]}")
else:
    print("::set-output name=matrix::{\"network\": [\"mandala\", \"karura\", \"acala\"]}")

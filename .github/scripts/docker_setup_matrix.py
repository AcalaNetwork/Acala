import sys
import inspect
import os

branch = sys.argv[1] if len(sys.argv) > 1 else ''

if branch.__contains__("release-"):
    chain, version = inspect.get_chain_and_version(branch)
    with open(os.getenv("GITHUB_OUTPUT"), "a") as file:
        matrix = "{\"network\": [\"" + chain + "\"]}"
        file.write(f"matrix={matrix}\n")
        file.write(f"version={version}\n")

else:
    with open(os.getenv("GITHUB_OUTPUT"), "a") as file:
        matrix = "{\"network\": [\"mandala\", \"karura\", \"acala\"]}"
        file.write(f"matrix={matrix}\n")

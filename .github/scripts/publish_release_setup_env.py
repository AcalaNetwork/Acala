import os
import inspect

chain, version = inspect.get_chain_and_version(os.getenv("GITHUB_REF"))
previous_version = inspect.get_previous_version(chain)

is_patch = previous_version.split(".")[1] == version.split(".")[1]
scope = "runtime" if is_patch else "full"

with open(os.getenv("GITHUB_ENV"), "a") as env:
    env.write("CHAIN={}\n".format(chain))
    env.write("SCOPE={}\n".format(scope))

import os
import inspect

chain, version = inspect.get_chain_and_version(os.getenv("GITHUB_REF"))
previous_version = inspect.get_previous_version(chain)

with open(os.getenv("GITHUB_ENV"), "a") as env:
	env.write("CHAIN={}\n".format(chain))
	env.write("VERSION={}\n".format(version))
	env.write("PREVIOUS_VERSION={}\n".format(previous_version))

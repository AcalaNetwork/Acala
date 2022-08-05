import os
import re

regex = r"release-(karura|acala)-(\d+\.\d+\.\d+)"

def get_chain_and_version(branch_name):
	# find chain and version from current branch
	x = re.search(regex, branch_name)
	chain = x.group(1)
	version = x.group(2)
	return chain, version

def get_previous_version(chain):
	cmd = os.popen("git branch -a | grep remotes/origin/release-{}-".format(chain));
	branches = cmd.read().split("\n")
	cmd.__exit__()
	branches = map(lambda x: x.strip(), branches)
	branches = list(filter(None, branches))
	# select previous branch
	previous_branches = branches[-2]

	# find previous version
	x = re.search(regex, previous_branches)
	return x.group(2)

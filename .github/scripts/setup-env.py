import os
import re

regex = r"release-(karura|acala)-(\d+\.\d+\.\d+)"

# find chain and version from current branch
x = re.search(regex, os.getenv("GITHUB_REF"))
chain = x.group(1)
version = x.group(2)

branches = os.popen("git branch -a | grep remotes/origin/release-{}-".format(chain)).read().split("\n")
branches = map(lambda x: x.strip(), branches)
branches = list(filter(None, branches))
# select previous branch
previous_branches = branches[-2]

# find previous version
x = re.search(regex, previous_branches)
previous_version = x.group(2)

is_patch = previous_version.split(".")[1] == version.split(".")[1]
scope = "runtime" if is_patch else "full"

with open(os.getenv("GITHUB_ENV"), "a") as env:
    env.write("CHAIN={}\n".format(chain))
    env.write("SCOPE={}\n".format(scope))

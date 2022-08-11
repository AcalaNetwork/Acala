import sys
import re

file = sys.argv[1] if len(sys.argv) > 1 else None
assert file, "Output file missing"

f = open(file, "r")

items = re.findall("\[\w+\] idx: \d+ -> \d+.+", f.read())

for item in items:
	print("::warning ::Index changed {}".format(item))

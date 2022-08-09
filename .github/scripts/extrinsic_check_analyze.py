import re

f = open("output.txt", "r")

items = re.findall("\[\w+\] idx: \d+ -> \d+.+", f.read())

for item in items:
	print("::warning ::Index changed {}".format(item))

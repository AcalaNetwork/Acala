import os
import unittest
import inspect

class InspectTestCase(unittest.TestCase):
	def setUp(self) -> None:
		os.environ["GITHUB_REF"] = "remotes/origin/release-karura-2.9.1"
		return super().setUp()

	def test_get_previous_version(self):
		assert inspect.get_previous_version("karura") == "2.8.3"

	def test_get_chain_and_version(self):
		chain, version = inspect.get_chain_and_version("remotes/origin/release-karura-2.9.1")
		assert chain == "karura"
		assert version == "2.9.1"

if __name__ == '__main__':
    unittest.main()

import { defineConfig } from 'vitest/config'
export default defineConfig({
	test: {
		hookTimeout: 240_000,
		testTimeout: 240_000,
		pool: 'forks',
		passWithNoTests: true,
		include: ['tests/**/test-*.{js,ts}'],
	},
})

// Deliberately independent of vite.config.ts: that config requires
// RAMEKIN_EXTERNAL_URL and shells out to git, neither of which unit
// tests should need. Vitest picks this file over vite.config.ts.
import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    // Exact matches: a bare 'solid-js' key would also rewrite 'solid-js/store'.
    // Both point at the client builds so signals and effects actually run.
    alias: [
      { find: /^solid-js$/, replacement: 'solid-js/dist/solid.js' },
      { find: /^solid-js\/store$/, replacement: 'solid-js/store/dist/store.js' },
    ],
  },
  test: {
    include: ['src/**/*.test.ts'],
  },
})

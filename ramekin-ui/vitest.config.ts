// Deliberately independent of vite.config.ts: that config requires
// RAMEKIN_EXTERNAL_URL and shells out to git, neither of which unit
// tests should need. Vitest picks this file over vite.config.ts.
import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
})

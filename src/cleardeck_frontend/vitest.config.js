// Unit tests for the frontend's PURE modules ($lib/*.js). Deliberately not the
// SvelteKit vite config: that one resolves canister ids for a build target and
// throws without one, and a unit test of a formatting function has no business
// needing a network.
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    alias: { $lib: new URL('./src/lib', import.meta.url).pathname },
  },
  test: {
    include: ['src/**/*.test.js'],
    environment: 'node',
  },
});

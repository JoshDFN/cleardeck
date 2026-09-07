// Unit tests for the frontend's PURE modules ($lib/*.js) and its rune modules
// ($lib/*.svelte.js). Deliberately not the SvelteKit vite config: that one
// resolves canister ids for a build target and throws without one, and a unit
// test of a formatting function has no business needing a network. The bare
// Svelte plugin (no kit) is here only so a `.svelte.js` module compiles; in
// the node environment it takes Svelte's server build, where `$state` is a
// plain variable and `$effect` is a no-op, which is exactly the surface a
// test of a door's ordering wants.
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [svelte({ configFile: false, hot: false })],
  resolve: {
    alias: { $lib: new URL('./src/lib', import.meta.url).pathname },
  },
  test: {
    include: ['src/**/*.test.js'],
    environment: 'node',
  },
});

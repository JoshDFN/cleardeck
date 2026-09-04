import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  // `<style lang="scss">` blocks may `@use` a partial next to the component, so
  // a component's token block or dock rules can live in a small file of their own
  // (PokerTable.svelte, +page.svelte) and still be scoped by Svelte.
  //
  // `configFile: false`: outside a vite build (svelte-check, the editor) the
  // preprocessor would otherwise load vite.config.js, whose network guard
  // aborts without DFX_NETWORK (docs/DEFECTS.md T-01). Sass needs no config;
  // a real build replaces this with its own resolved config anyway.
  preprocess: vitePreprocess({ script: false, style: { configFile: false } }),
  kit: {
    // adapter-auto only supports some environments, see https://kit.svelte.dev/docs/adapter-auto for a list.
    // If your environment is not supported or you settled on a specific environment, switch out the adapter.
    // See https://kit.svelte.dev/docs/adapters for more information about adapters.
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
      precompress: false,
      strict: false,
    }),
  },
};

export default config;

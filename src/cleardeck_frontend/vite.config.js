import { fileURLToPath, URL } from 'url';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import environment from 'vite-plugin-environment';
import { prepareBuildEnv } from './build/network-env.mjs';

// docs/DEFECTS.md T-01. This line used to be an unconditional
//   dotenv.config({ path: '../../.env' })
// and that .env holds the MAINNET canister ids, so a bare `npm run build`
// silently wired a local dev bundle to the live fund-holding canisters.
//
// prepareBuildEnv() replaces it: it requires the target network to be stated,
// loads the repo-root .env only for an `ic` build, and throws with an actionable
// message if a local build resolves a mainnet id or no id at all. Throwing here
// aborts the vite build before a single byte is emitted.
const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
const buildTarget = prepareBuildEnv({ repoRoot: REPO_ROOT });
console.log(
  `[cleardeck] frontend build target: network=${buildTarget.network} ` +
    `lobby=${buildTarget.ids.LOBBY} history=${buildTarget.ids.HISTORY} ` +
    `(mainnet denylist: ${buildTarget.denylistSize} ids from ${buildTarget.denylistSource}` +
    `${buildTarget.envFileLoaded ? `; loaded ${buildTarget.envFileLoaded}` : ''})`,
);

export default defineConfig({
  build: {
    emptyOutDir: true,
  },
  optimizeDeps: {
    esbuildOptions: {
      define: {
        global: "globalThis",
      },
    },
  },
  server: {
    proxy: {
      // docs/DEFECTS.md T-03: this project's managed network gateway is pinned to
      // 8077 in icp.yaml (8000 belongs to another project on the same machine),
      // so a hardcoded 4943 proxied `vite dev` to a port nothing listens on.
      "/api": {
        target: `http://127.0.0.1:${Number(process.env.VITE_LOCAL_GATEWAY_PORT) || 4943}`,
        changeOrigin: true,
      },
    },
  },
  plugins: [
    sveltekit(),
    // Legacy dfx wrote CANISTER_ID_* / DFX_*. The deploy scripts export
    // VITE_CANISTER_ID_* (icp-cli has no dfx-style .env writer). Expose all three
    // prefixes so the build resolves canister IDs from process.env or ../../.env.
    environment("all", { prefix: "CANISTER_" }),
    environment("all", { prefix: "DFX_" }),
    environment("all", { prefix: "VITE_" }),
  ],
  resolve: {
    alias: [
      {
        find: "declarations",
        replacement: fileURLToPath(
          new URL("../declarations", import.meta.url)
        ),
      },
    ],
    dedupe: ['@dfinity/agent'],
  },
});

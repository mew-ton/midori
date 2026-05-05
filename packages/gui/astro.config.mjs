import node from "@astrojs/node";
import { defineConfig } from "astro/config";

// SSR via the standalone Node adapter. The dev server on 127.0.0.1:4321
// is what `electron/main.js` loads in dev mode. Loopback (not 0.0.0.0)
// keeps the renderer locally addressable without exposing the SSR server
// to the network. The built `dist/server/entry.mjs` is the production
// SSR bundle that the main process will spawn; that path is deferred
// and intentionally not wired in this scaffold.
export default defineConfig({
  output: "server",
  adapter: node({ mode: "standalone" }),
  server: { port: 4321, host: "127.0.0.1" },
});

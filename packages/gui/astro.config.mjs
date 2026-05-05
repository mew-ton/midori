import node from "@astrojs/node";
import { defineConfig } from "astro/config";

// SSR via the standalone Node adapter, matching design/03-tech-stack.md:
// "Electron のメインプロセスでローカル Node サーバー（Astro SSR）を起動し、
// レンダラーは http://localhost:PORT を表示する". The dev server on
// 127.0.0.1:4321 is what `electron/main.js` loads in dev mode; the built
// `dist/server/entry.mjs` is what the production main process will spawn
// (wired in a follow-up Issue).
export default defineConfig({
  output: "server",
  adapter: node({ mode: "standalone" }),
  server: { port: 4321, host: "127.0.0.1" },
});

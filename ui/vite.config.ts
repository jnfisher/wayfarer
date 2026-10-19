import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// `npm run dev` serves the UI with hot reload and proxies the WebSocket to a running gateway
// (docs/plan/07-development-and-testing.md, "Four restart domains"). `npm run build` writes
// dist/, which `wayfarer gateway` serves.
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      "/ws": { target: "ws://127.0.0.1:7373", ws: true },
    },
  },
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
  test: {
    include: ["tests/**/*.test.ts"],
  },
});

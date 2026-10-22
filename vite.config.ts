import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The front end lives in ui/; the Tauri shell loads dist/ (app/tauri.conf.json).
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  // morphe ships Svelte and TypeScript sources: the dev server's dependency
  // pre-bundling cannot read them, so they go through the Svelte plugin.
  optimizeDeps: { exclude: ["morphe"] },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/app/**", "**/core/**", "**/target/**"] },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2022",
  },
});

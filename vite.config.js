import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: { target: "esnext" },
  server: { port: 1430, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
});

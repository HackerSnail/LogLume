import { defineConfig } from "vite";

// Fixed dev port kept separate from NoteLume's 1420 so both apps can run at
// once. `strictPort` fails loudly rather than drifting to a port the Tauri
// config wouldn't know about.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1520,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    target: "es2021",
  },
});

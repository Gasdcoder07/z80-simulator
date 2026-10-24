import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built files from ../dist and drives the dev server on a
// fixed port, so nothing here may pick a different one.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: {
    target: "safari15",
    outDir: "dist",
    sourcemap: true,
  },
});

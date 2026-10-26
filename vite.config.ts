import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and must not clear its own terminal output.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // Do not watch src-tauri: cargo locks files in target/ while building (EBUSY).
  server: { port: 1420, strictPort: true, host: "127.0.0.1", watch: { ignored: ["**/src-tauri/**"] } },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "chrome110", outDir: "dist", emptyOutDir: true },
});

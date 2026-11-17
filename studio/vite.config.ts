import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The Rust engine owns the document; this app is only a view of it.
// The proxy keeps the browser on one origin during development.
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: "http://127.0.0.1:8787", changeOrigin: false },
    },
  },
});

import { defineConfig } from "vite";
import { resolve } from "node:path";

// Vários pontos de entrada (popup e painel entram em marcos futuros).
export default defineConfig({
  root: "src",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "chrome105", // WebView2
    rollupOptions: {
      input: {
        panel: resolve(import.meta.dirname, "src/panel.html"),
        popup: resolve(import.meta.dirname, "src/popup.html"),
      },
    },
  },
});

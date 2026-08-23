import path from "node:path";
import { defineConfig } from "vitest/config";

// Kept separate from `vite.config.ts` because that one is written for the Tauri
// dev server — a fixed port, a Rust-aware watcher — none of which a test run
// wants, and its async default export cannot carry a `test` block cleanly.
export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    // No DOM: what is worth testing here is the store's dealings with the
    // daemon, not React's rendering of them.
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});

import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    // Application tests live only under `src/**`. Vitest's default include
    // pattern also sweeps `benchmarks/.vendor/**`, the gitignored upstream
    // runtime checkouts Spec 05 clones locally (whisper.cpp ships its own
    // `__test__/whisper.spec.js` that requires a native addon build), which
    // made `npm test` fail on any machine that had run a benchmark build.
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
});

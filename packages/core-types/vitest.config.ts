import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    coverage: {
      provider: "v8",
      // index.ts exports only type/interface declarations — erased at compile
      // time, nothing for v8 to instrument. Exclude it so coverage stays clean.
      exclude: ["src/index.ts", "vitest.config.ts"],
    },
  },
});

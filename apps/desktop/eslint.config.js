// Flat ESLint config for @tally/desktop.
//
// Scope: this config exists primarily to enforce that no production code
// imports `invoke` directly from `@tauri-apps/api/core`. All call sites must
// route through `safeInvoke` / `safeInvokeOrAdvise` in `src/lib/safeInvoke.ts`,
// which is the only file allowed to do that direct import (and carries an
// explicit `eslint-disable-next-line` for the line in question).
//
// Type-only imports of `invoke` (e.g. `import type { invoke } from ...`) are
// allowed — those don't pull the value into the runtime.

import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import reactHooks from "eslint-plugin-react-hooks";

export default [
  {
    ignores: [
      "dist/**",
      "coverage/**",
      "node_modules/**",
      "src-tauri/**",
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    linterOptions: {
      // Pre-existing inline disable directives target rules we haven't
      // turned on (e.g. `react-hooks/exhaustive-deps`, `no-empty`). Don't
      // surface them as warnings — that's out of Task 14 scope.
      reportUnusedDisableDirectives: "off",
    },
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      "@typescript-eslint": tsPlugin,
      "react-hooks": reactHooks,
    },
    rules: {
      // The hook is only registered so that pre-existing
      // `// eslint-disable-next-line react-hooks/exhaustive-deps` directives
      // in the codebase don't surface as "unused disable" warnings. We don't
      // want to expand lint scope beyond Task 14 here.
      "react-hooks/exhaustive-deps": "off",
      "@typescript-eslint/no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "@tauri-apps/api/core",
              importNames: ["invoke"],
              message:
                "Use safeInvoke / safeInvokeOrAdvise from src/lib/safeInvoke.ts.",
              allowTypeImports: true,
            },
          ],
        },
      ],
    },
  },
  {
    // Test files mock `@tauri-apps/api/core` via vi.mock and need to import
    // `invoke` purely to drive the mock. Allow the direct import in tests.
    files: ["src/**/*.test.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-restricted-imports": "off",
    },
  },
];

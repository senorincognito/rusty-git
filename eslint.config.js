import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

// Only the two classic hook rules: the plugin's React Compiler rules flag patterns this code uses on
// purpose (resetting state when `path` changes, syncing a ref during render).
export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules"] },
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
    },
    languageOptions: { globals: globals.browser },
  },
);

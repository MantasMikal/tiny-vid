import { defineConfig, globalIgnores } from "eslint/config";
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import reactCompiler from "eslint-plugin-react-compiler";
import simpleImportSort from "eslint-plugin-simple-import-sort";
import unusedImports from "eslint-plugin-unused-imports";
import noRelativeImportPaths from "eslint-plugin-no-relative-import-paths";
import betterTailwindcss from "eslint-plugin-better-tailwindcss";
import eslintConfigPrettier from "eslint-config-prettier/flat";

const tsAndTsx = ["**/*.ts", "**/*.tsx"];
const srcOnly = ["src/**/*.ts", "src/**/*.tsx"];

export default defineConfig([
  // 1. Global ignores (Rust, build outputs, deps)
  globalIgnores([
    "src-tauri/**",
    "dist/**",
    "**/node_modules/**",
    "build/**",
    ".yarn/**",
    "eslint.config.ts",
  ]),

  // 2. Base JS recommended (only for JS/TS so CSS is not parsed as JS)
  { files: ["**/*.js", "**/*.ts", "**/*.jsx", "**/*.tsx"], ...js.configs.recommended },

  // 2b. Node globals for root config files (vite, etc.) – excluded from strict type-check
  {
    files: ["vite.config.ts"],
    languageOptions: {
      globals: {
        process: "readonly",
        __dirname: "readonly",
        __filename: "readonly",
        module: "readonly",
        require: "readonly",
      },
    },
  },

  // 3. TypeScript strict + type-checked (applies to TS/TSX)
  {
    files: tsAndTsx,
    ignores: ["vite.config.ts"],
    extends: [
      tseslint.configs.strict,
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
    ],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
      globals: {
        window: "readonly",
        document: "readonly",
        console: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
        fetch: "readonly",
        Request: "readonly",
        Response: "readonly",
        URL: "readonly",
        Blob: "readonly",
        FormData: "readonly",
        AbortController: "readonly",
        __TAURI__: "readonly",
      },
    },
  },

  // 4. React + Hooks + React Compiler (TSX/JSX)
  {
    files: ["**/*.tsx", "**/*.jsx"],
    extends: [
      react.configs.flat.recommended,
      react.configs.flat["jsx-runtime"],
      reactHooks.configs.flat.recommended,
      reactCompiler.configs.recommended,
    ],
    settings: {
      react: {
        version: "detect",
      },
    },
  },

  // 5. Unused imports (auto-fix) + import sorting
  {
    files: tsAndTsx,
    plugins: {
      "simple-import-sort": simpleImportSort,
      "unused-imports": unusedImports,
    },
    rules: {
      "@typescript-eslint/no-unused-vars": "off",
      "unused-imports/no-unused-imports": "error",
      "unused-imports/no-unused-vars": [
        "warn",
        {
          vars: "all",
          varsIgnorePattern: "^_",
          args: "after-used",
          argsIgnorePattern: "^_",
        },
      ],
      "simple-import-sort/imports": "error",
      "simple-import-sort/exports": "error",
    },
  },

  // 6. No relative imports (src only, use @/ alias)
  {
    files: srcOnly,
    plugins: {
      "no-relative-import-paths": noRelativeImportPaths,
    },
    rules: {
      "no-relative-import-paths/no-relative-import-paths": [
        "error",
        { allowSameFolder: false, rootDir: "src", prefix: "@" },
      ],
    },
  },

  // 7. Better Tailwind CSS (Tailwind 4 entry: app.css with @theme; only TSX/JSX, not raw CSS)
  {
    files: ["**/*.tsx", "**/*.jsx"],
    extends: [betterTailwindcss.configs.recommended],
    settings: {
      "better-tailwindcss": {
        entryPoint: "src/styles/app.css",
      },
    },
  },

  // 7b. WindowFrame uses custom window-content class for scrollbar – allow unknown classes
  {
    files: ["src/components/window/WindowFrame.tsx"],
    rules: {
      "better-tailwindcss/no-unknown-classes": "off",
    },
  },

  // 8. Prettier last (disables conflicting rules)
  eslintConfigPrettier,
]);

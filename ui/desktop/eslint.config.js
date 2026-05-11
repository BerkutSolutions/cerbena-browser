export default [
  {
    files: [
      "web/app/**/*.js",
      "web/app/__tests__/*.mjs"
    ],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        window: "readonly",
        document: "readonly",
        navigator: "readonly",
        console: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
        URL: "readonly",
        Blob: "readonly",
        FileReader: "readonly",
        fetch: "readonly",
        process: "readonly",
        localStorage: "readonly",
        crypto: "readonly",
        structuredClone: "readonly",
        btoa: "readonly",
        URLSearchParams: "readonly",
        requestAnimationFrame: "readonly"
      }
    },
    rules: {
      "no-undef": "error",
      "no-unused-vars": ["error", { "argsIgnorePattern": "^_", "varsIgnorePattern": "^_" }],
      "no-unreachable": "error"
    }
  }
];

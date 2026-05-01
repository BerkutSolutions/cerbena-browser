---
title: Docs Site and Wiki Build
sidebar_position: 2
---

Cerbena Browser documentation is built through the root-level `Docusaurus` configuration.

## Structure

- `https://github.com/BerkutSolutions/cerbena-browser` - GitHub repository
- `/` - docs portal home
- `/ru/` - Russian wiki
- `/en/` - English wiki
- `docs/ru` - Russian branch
- `docs/eng` - English branch
- `sidebars.ru.js` and `sidebars.en.js` - navigation
- `docusaurus.config.js` - site configuration

## Commands

```bash
npm install
npm run docs:start
npm run docs:build
```

## Rules

- new pages should land in both locales;
- `README` files act as static indexes outside the live wiki runtime;
- docs quality tests must stay green.

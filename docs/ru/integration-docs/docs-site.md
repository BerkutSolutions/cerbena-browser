---
title: Docs-site и сборка wiki
sidebar_position: 2
---

Документация Cerbena Browser собирается `Docusaurus`-конфигурацией из корня репозитория.

## Структура

- `https://github.com/BerkutSolutions/cerbena-browser` - GitHub репозиторий
- `/` - главная портала документации
- `/ru/` - русская wiki
- `/en/` - английская wiki
- `docs/ru` - русская ветка
- `docs/eng` - английская ветка
- `sidebars.ru.js` и `sidebars.en.js` - навигация
- `docusaurus.config.js` - конфиг сайта

## Команды

```bash
npm install
npm run docs:start
npm run docs:build
```

## Правила

- новые страницы должны появляться в обеих локалях;
- `README`-файлы служат индексами вне runtime-ветки;
- тесты качества документации должны оставаться зелеными.

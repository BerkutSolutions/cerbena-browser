---
title: Контрольные проверки релиза
sidebar_position: 3
---

Релиз не должен опираться только на успешную сборку.

## Минимальные проверки

- `cargo test --workspace`
- docs quality tests
- `npm run docs:build`
- `cd ui/desktop && npm test`
- `powershell -ExecutionPolicy Bypass -File scripts/security-gates-preflight.ps1`
- `powershell -ExecutionPolicy Bypass -File scripts/vulnerability-gates-preflight.ps1`
- проверка `README.md`, `README.en.md`, `docs/ru`, `docs/eng`
- просмотр [Проверок безопасности](./security-validation.md), если заметки о выпуске ссылаются на покрытие hardening-задач `TASKS4` / `U14-2`

## Что дает такой подход

- защита от разрыва между кодом и wiki;
- защита от сломанной локализации;
- контроль над базовыми launcher-сценариями;
- привязка заявлений о защите для `TASKS4` к явной проверке остаточных рисков, а не только к описанию;
- более предсказуемый цикл выпуска.

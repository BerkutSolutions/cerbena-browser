---
title: Диагностика и частые проблемы
sidebar_position: 12
---

## Профиль не запускается

- проверьте подтверждение `Wayfern ToS`;
- проверьте, существует ли путь к binary;
- проверьте operator/runtime diagnostics на ошибки provisioning и запуска.

## Трафик блокируется неожиданно

- откройте `Traffic` и проверьте reason;
- проверьте kill-switch и глобальный `block without VPN`;
- проверьте `selected_services`, blocklists и domain deny rules.

## Не стартует route runtime

- посмотрите, какой backend нужен: `sing-box`, `openvpn`, `amneziawg` или `tor`;
- проверьте, прошла ли managed provisioning-фаза;
- проверьте ошибки в runtime-логах профиля.

## Есть проблемы с sync

- откройте `Settings > Sync`;
- проверьте текущий статус и endpoint health;
- убедитесь, что у профиля есть snapshots перед restore.

## Пропали локализации

- выполните `cd ui/desktop && npm run i18n:check`;
- проверьте паритет ключей `ru` и `en`;
- проверьте отсутствие mojibake в русских строках.

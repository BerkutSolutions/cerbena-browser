---
title: Managed runtime
sidebar_position: 1
---

Cerbena Browser умеет подготавливать и использовать managed runtime-бинарники для сетевого контура.

## Что сюда входит

- browser runtimes для `Wayfern` и `Camoufox`;
- route backends `sing-box`, `openvpn`, `amneziawg`, `tor`;
- локальное кеширование, проверка целостности и повторное использование артефактов.

## Операторские задачи

- проверять, что binary provisioning завершился успешно;
- отслеживать ошибки route runtime через operator/runtime diagnostics и профильные runtime-логи;
- понимать, какой backend поднимается для конкретного profile template.

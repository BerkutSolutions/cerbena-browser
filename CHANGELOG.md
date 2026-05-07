# Журнал изменений

## 1.1.3

### Ядро
- Защищен MSI release-контур от случайного расщепления product-линеек: `build-installer.ps1` теперь игнорирует `CERBENA_MSI_UPGRADE_CODE`, если не включен явный тестовый флаг `CERBENA_ALLOW_CUSTOM_MSI_UPGRADE_CODE=1`; `local-updater-e2e.ps1` включает этот флаг только внутри изолированного snapshot-build и корректно откатывает переменные окружения.

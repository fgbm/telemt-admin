# ADR 004: источники конфигурации, env-overrides и Docker defaults

## Статус

Принято

## Контекст

Конфигурация исторически задаётся файлом `telemt-admin.toml`. Для Docker и оркестраторов удобно переопределять отдельные параметры через переменные окружения без пересборки файла. При этом нельзя:

- превратить env в второй полный источник истины рядом с TOML;
- запекать секреты в `Dockerfile` через `ENV`/`ARG` (они остаются в слоях образа и метаданных);
- смешивать неявные «image defaults» из `ENV` с runtime overrides без явной policy.

## Решение

1. **Каноническая схема** остаётся TOML: структура `Config` и секции как в файле.

2. **Runtime env-overrides** с фиксированным префиксом `TELEMT_ADMIN__` применяются **после** чтения TOML и **перезаписывают** только явно поддерживаемые поля (whitelist).

3. **Секреты** (`bot_token`, `telemt_api.auth_header`) допускаются в env-overrides, но **не** задаются в `Dockerfile` через `ENV`/`ARG`.

4. **Legacy alias** `TELOXIDE_TOKEN` сохраняется: если `bot_token` в итоговой конфигурации не задан, используется `TELOXIDE_TOKEN`.

5. **Docker-friendly defaults** для приложения не запекаются как «магические» `ENV` в образе; вместо этого поставляется **пример TOML** в образе: `/usr/share/doc/telemt-admin/docker-default.toml.example` (копия из репозитория). Операционный `RUST_LOG=info` в `ENV` допустим как несекретный.

6. **Precedence** при формировании итогового `Config`:

   1. значения из TOML (включая serde default-ы для отсутствующих ключей в файле);
   2. whitelist `TELEMT_ADMIN__*` overrides;
   3. при разрешении токена бота: если `bot_token` всё ещё пуст — `TELOXIDE_TOKEN`.

7. **Логирование**: фиксируется только **список имён** применённых env-ключей и их количество, без значений.

## Whitelist `TELEMT_ADMIN__*`

Поддерживаются (суффикс после префикса `TELEMT_ADMIN__`):

- `BOT_TOKEN`
- `BOT_USERNAME` (username бота без `@`, если `getMe` недоступен)
- `ADMIN_IDS` (список через запятую)
- `TELEMT_CONFIG_PATH`
- `DB_PATH`
- `SERVICE_NAME` (верхнеуровневый `service_name` в TOML)
- `RUNTIME__MODE` (`systemd` | `external` | `none`)
- `RUNTIME__SERVICE_NAME`
- `RUNTIME__LABEL`
- `TELEMT_API__ENABLED`
- `TELEMT_API__BASE_URL`
- `TELEMT_API__AUTH_HEADER`
- `TELEMT_API__TIMEOUT_MS`
- `TELEMT_API__ALLOW_FILE_FALLBACK`
- `TELEMT_API__PREFER_API_LINKS`
- `NOTIFICATIONS__ENABLED`
- `NOTIFICATIONS__HEALTH_CHECK_INTERVAL_SECS`
- `NOTIFICATIONS__NOTIFY_ON_HEALTH_CHANGE`
- `NOTIFICATIONS__NOTIFY_ON_RUNTIME_ALERTS`
- `NOTIFICATIONS__NOTIFY_ON_NEW_REQUEST`
- `WEB_PROXY__HOST`
- `WEB_PROXY__SECRET_MODE` (`dd` | `plain`)
- `WEBAPP__ENABLED`
- `WEBAPP__LISTEN` (в контейнере — `0.0.0.0:8090` за reverse proxy)
- `WEBAPP__PUBLIC_URL`

Расширение whitelist — отдельное изменение с обновлением ADR и README.

## Последствия

- Docker Compose может задавать чувствительные значения через `environment`/`env_file`, не копируя их в образ.
- Поведение на классическом Linux + systemd без env-overrides не меняется.
- Пользователи должны знать, что env-overrides имеют приоритет над TOML для перечисленных полей.

## Инварианты

- Нет полного 1:1 mirror всех полей TOML в env без явного решения.
- Секреты не попадают в `Dockerfile` как `ENV`/`ARG`.

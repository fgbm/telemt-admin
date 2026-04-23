# Runbook: rollout и эксплуатация `telemt` control API

## Назначение

Этот документ описывает, как включать и проверять API-first интеграцию `telemt-admin` с `telemt`, а также как откатываться на legacy-путь при проблемах.

## Docker / `runtime = external`

Если бот запущен в контейнере без доступа к host systemd:

- задайте в `telemt-admin.toml` секцию `[runtime]` с `mode = "external"` (или `none`);
- убедитесь, что `[telemt_api].base_url` указывает на реальный адрес control API (имя сервиса в Docker-сети, а не только `127.0.0.1`);
- для чисто API-сценария рекомендуется `allow_file_fallback = false`.
- текущий пример Compose предполагает именно API-only путь: конфиг `telemt-admin` монтируется read-only, а общий write-path к `telemt.toml` не настраивается;
- если нужен legacy fallback с записью в `telemt.toml`, потребуется отдельный RW mount/volume и явное понимание, что этот сценарий не покрывается примером `deploy/compose/docker-compose.telemt-admin.example.yml`.

Кнопки start/stop/restart/reload на экране `/service` в этих режимах скрыты; диагностика идёт через control API.

Переменные `TELEMT_ADMIN__*` (whitelist) применяются после TOML и удобны для Compose; см. `docs/adr/004-config-sources-and-docker-defaults.md`.

## Группы, рассылка, импорт и тексты бота

- **Группы**: таблицы `user_groups` и `user_group_members`; опциональный общий срок группы — `user_groups.expires_at` (unix). В UI доступны сценарии «задать/изменить срок», «снять срок» и «применить срок группы ко всем»; последний выполняет PATCH `expiration` для участников (нужен включённый `[telemt_api]`).
- **Рассылка**: одно сообщение всем пользователям со статусом `approved`; учитывайте лимиты Telegram и то, что бот не доставит сообщение пользователям, которые ни разу не взаимодействовали с ботом.
- **Импорт из telemt**: по числовому Telegram user id, только при `telemt_api.enabled`; локальная запись создаётся без сохранённого секрета (ссылки строятся через control API при `prefer_api_links` и т.п.).
- **Автоподхват existing user**: если локальной записи ещё нет, но пользователь уже существует в `telemt API` как `tg_<id>`, user flow (`/start`, `/link`, invite) может автоматически импортировать его в локальную БД. Для invite-перехода такого пользователя `usage_count` не увеличивается.
- **Тексты**: секция `[bot_messages]` и env `TELEMT_ADMIN__BOT_MESSAGES__*` (см. `src/config.rs`). Помимо `/start`, можно переопределять шаблон ссылки `{link}`, текст авто-одобрения и сообщения вокруг рассылки.

## Предусловия

На стороне `telemt`:

- включён `[server.api]`;
- задан безопасный `listen`, обычно `127.0.0.1:9091`;
- настроен `auth_header`;
- при необходимости настроен `whitelist`.

На стороне `telemt-admin`:

- задана секция `[telemt_api]`;
- `base_url` указывает на фактический bind control API;
- `auth_header` в точности совпадает с `server.api.auth_header`;
- если используется фоновый мониторинг, настроена секция `[notifications]`.

## Рекомендуемая конфигурация

Пример `telemt.toml`:

```toml
[server.api]
enabled = true
listen = "127.0.0.1:9091"
whitelist = ["127.0.0.1/32", "::1/128"]
auth_header = "Bearer <generated-secret>"
```

Пример `telemt-admin.toml`:

```toml
[telemt_api]
enabled = true
base_url = "http://127.0.0.1:9091"
auth_header = "Bearer <generated-secret>"
timeout_ms = 5000
allow_file_fallback = true
prefer_api_links = true

[notifications]
enabled = true
health_check_interval_secs = 60
notify_on_health_change = true
notify_on_runtime_alerts = true
notify_on_new_request = true
```

## Порядок rollout

### Фаза 1. Подготовка

- задеплоить версию `telemt-admin` с `telemt_backend`;
- оставить fallback включённым;
- проверить, что legacy-путь всё ещё работает.

### Фаза 2. Включение API backend

- настроить `[server.api]` в `telemt`;
- выполнить restart `telemt`, если менялись параметры `server.api`;
- включить `[telemt_api].enabled = true` в `telemt-admin`;
- проверить или задать политику `[notifications]` в `telemt-admin`;
- перезапустить `telemt-admin`.

### Фаза 3. Подтверждение работоспособности

Проверить вручную:

1. создать пользователя через auto-approve токен;
2. вручную одобрить pending-заявку;
3. запросить `/link` для уже существующего пользователя;
4. удалить пользователя;
5. открыть `/service` и убедиться, что видны:
   - systemd state;
   - health control API;
   - runtime/system info;
   - security posture;
   - блок нагрузки (`uptime`, connections, bad connections, handshake timeouts);
   - live connections (`total`, `ME`, `Direct`, `active users`);
   - экран `Top пользователей`.
6. открыть карточку пользователя и убедиться, что:
   - подгружаются live runtime-данные;
   - кнопки изменения лимитов работают через control API.
7. при включённых `[notifications]` проверить, что новые manual-заявки и health/runtime alerts приходят администраторам.

## Ожидаемое поведение fallback

Если `allow_file_fallback = true`, при сбое control API допустимо следующее:

- approve/create выполняется через legacy-конфиг и `systemd`;
- `/link` собирается локально по `secret` из SQLite;
- delete идёт через старый путь редактирования `telemt.toml`.

Fallback не должен быть скрытым operationally:

- причина деградации должна отражаться в логах;
- sync-метаданные пользователя в SQLite должны помогать понять, что произошло.

## Значение sync-полей в SQLite

- `backend_mode` — какой backend последним успешно применял изменения;
- `last_sync_error` — последний стабильный код деградации или ошибки синхронизации;
- `last_seen_revision` — последняя revision, полученная от control API;
- `last_synced_at` — время последней фиксации sync-состояния.

Семантика записи:

- `backend_mode = control_api` означает, что последнее успешное изменение было применено через control API без ухода на legacy-путь;
- `backend_mode = legacy_file` означает, что последнее успешное изменение было применено через локальный `telemt.toml`/legacy-path;
- `last_seen_revision` заполняется только если сценарий действительно завершился через control API и revision была получена;
- `last_sync_error = NULL` означает, что последнее изменение завершилось без известной деградации, которую нужно подсветить оператору;
- `last_sync_error` не заменяет логи: это короткий стабильный operational code для UI и диагностики.

## Стабильные sync error codes

На текущем этапе проект использует и резервирует следующие коды:

- `degraded_legacy_fallback` — операция должна была идти через control API, но была успешно применена через legacy fallback;
- `user_not_found_in_backend` — локальная запись есть, но удаление/сверка не нашли пользователя в backend;
- `degraded_link_fallback` — резерв под сценарии, где данные пользователя получены, но ссылку пришлось строить локально;
- `api_error_timeout` — резерв под явную фиксацию timeout от control API;
- `api_error_auth` — резерв под ошибки авторизации control API;
- `api_error_revision_conflict` — резерв под конфликты revision/`If-Match`;
- `api_error_not_found` — резерв под сценарии not found на control API.

Текущая policy:

- approve и auto-approve очищают `last_sync_error`, если provisioning завершился по чистому API-path;
- approve и auto-approve пишут `degraded_legacy_fallback`, если проект был в режиме `API-first`, но provisioning фактически ушёл в legacy fallback;
- hard ban пишет `user_not_found_in_backend`, если локальный approved-пользователь уже не найден в backend на этапе удаления;
- hard ban пишет `degraded_legacy_fallback`, если delete должен был завершиться через control API, но фактически был выполнен через legacy fallback.

## Фоновый мониторинг и уведомления

Секция `[notifications]` управляет фоновым polling `telemt` API в `telemt-admin`.

- `enabled = true` включает фоновую задачу мониторинга;
- `health_check_interval_secs` задаёт интервал опроса;
- `notify_on_health_change` включает уведомления о:
  - недоступности/восстановлении control API;
  - смене health status;
  - смене `accepting_new_connections`;
  - смене `me_runtime_ready`;
- `notify_on_runtime_alerts` включает уведомления о:
  - появлении/исчезновении `unhealthy upstream`;
  - проблемах и восстановлении `ME self-test` (`KDF`, `time skew`);
- `notify_on_new_request` включает уведомления администраторам о новых manual-заявках.

Operational note:

- если на staging/production уведомления слишком шумные, сначала уменьшайте scope через флаги `notify_on_*`, а не выключайте весь `[telemt_api]`;
- при `notifications.enabled = false` бот остаётся полностью рабочим, но перестаёт слать push-уведомления и health/runtime alerts;
- идентичные уведомления не дублируются чаще чем раз в 5 минут: если состояние флапает, админ получит первый алерт, а повторы будут подавлены до смены текста или истечения таймаута.

## Частичная деградация control API

Экран `/service` не должен считаться бинарным индикатором «всё работает / всё сломано».

- systemd/host-runtime, локальная БД, sync-агрегаты и события админа могут отображаться даже если часть control API endpoints недоступна;
- блок `Демон (control API)` зависит от runtime snapshot;
- блок `Нагрузка` зависит от `stats_summary`;
- строки `Live` и экран `Top пользователей` зависят от `connections_summary`;
- если `/v1/runtime/connections/summary` недоступен (например, `runtime_edge_enabled=false`), бот автоматически делает fallback на `/v1/stats/users` и строит top-N самостоятельно;
- если конкретный endpoint недоступен, UI показывает причину ошибки именно в соответствующем блоке, а не скрывает весь экран.

Что проверять оператору:

- если отсутствует только блок `Демон (control API)` — проверить `/v1/health`, `/v1/system/info`, `/v1/runtime/*`, auth header и whitelist;
- если падает только `Нагрузка` или `Top пользователей` — проверить runtime endpoints `/v1/stats/summary` и `/v1/runtime/connections/summary`;
- если в `/service` виден рост `Sync: degraded` или появляются `Sync ошибки`, смотреть коды `last_sync_error` и correlate их с логами fallback/API;
- если Telegram-уведомления приходят, а `/service` показывает частичную деградацию, это ожидаемо: монитор планирует сигналы отдельно от рендеринга UI и не требует полной доступности всех endpoints.

## Когда нужен restart `telemt`

Restart обязателен, если менялись параметры `[server.api]`, включая:

- `enabled`;
- `listen`;
- `whitelist`;
- `auth_header`.

Обычные CRUD-операции над пользователями через control API не должны требовать restart.

## Известные ограничения

- `POST /v1/users/{username}/rotate-secret` в текущем `telemt` не использовать;
- `/metrics` не заменяет control API;
- наличие fallback не должно оправдывать публичную экспозицию control API без auth.
- при `DELETE /v1/users/{username}` ответ `404` трактуется как `user_not_found_in_backend` и не инициирует legacy delete-path автоматически; это защищает от лишнего удаления, но оставляет возможный residual risk для старых legacy-only записей.
- fallback-запись `telemt.toml` старается быть атомарной через temp-file + rename; при `PermissionDenied` на создание temp-файла переходит на прямую запись в целевой файл; при cross-device rename (`EXDEV`) используется fallback `copy + remove`;
- это осознанный reliability-компромисс для окружений вроде `/etc` и контейнеров с overlay-fs.
- `Mutex` внутри `TelemtConfig` сериализует доступ только внутри одного процесса `telemt-admin`; внешние редакторы файла и параллельные писатели вне процесса остаются вне модели.
- self-update выполняет распаковку и замену бинарника в отдельном blocking-контуре, проверяя SHA-256 checksum из релизного `.sha256`-файла; при несовпадении обновление отменяется;
- не предоставляет полноценный application-level rollback при сбое замены после верификации.

## Откат

Если rollout проходит неуспешно:

1. установить `[telemt_api].enabled = false` в `telemt-admin.toml`;
2. при необходимости отключить `[notifications].enabled`, чтобы убрать фоновые API health-check;
3. оставить `telemt` работающим в прежнем режиме;
4. перезапустить `telemt-admin`;
5. убедиться, что approve, `/link` и delete снова работают через legacy-path.

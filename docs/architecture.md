# Архитектура

Основные модули:

- `src/main.rs` — инициализация конфига, БД, состояния бота и `Dispatcher`; graceful shutdown по `Ctrl-C` с закрытием пула SQLite.
- `src/config.rs` — загрузка `telemt-admin.toml`, дефолты и валидация (в т.ч. `[bot_messages]`).
- `src/env_config_overlay.rs` — whitelist overrides `TELEMT_ADMIN__*` поверх TOML.
- `src/monitor.rs` — фоновый polling `telemt` control API и push-уведомления администраторам; дедупликация одинаковых алертов с интервалом 5 минут.
- `src/db.rs` — корневой модуль SQLite-слоя с общими типами и публичным API.
- `src/db/migrations.rs` — мягкие миграции схемы и bootstrap БД.
- `src/db/registration.rs` — заявки, пользователи и переходы `pending/approved/rejected/deleted`.
- `src/db/invite_tokens.rs` — invite-токены, consume/revoke и проверки активного токена.
- `src/db/user_groups.rs` — группы пользователей и членство (`user_groups`, `user_group_members`).
- `src/db/admin.rs` — агрегаты админки и структурированные события активности.
- `src/db/wizard_state.rs` — хранение wizard-state и TTL cleanup.
- `src/telemt_cfg.rs` — legacy-адаптер для чтения и изменения `telemt.toml`; блокирующий файловый I/O должен выполняться через offload, а не напрямую в async bot flow.
- `src/telemt_backend.rs` — публичный фасад backend-слоя `telemt`, который сохраняет единый API для остального кода.
- `src/telemt_backend/api_client.rs` и `src/telemt_backend/api_dto.rs` — HTTP-клиент control API, revision/retry и приватные DTO ответов.
- `src/telemt_backend/control_api.rs`, `src/telemt_backend/legacy.rs`, `src/telemt_backend/mappers.rs`, `src/telemt_backend/types.rs` — реализация API-first backend, legacy fallback, чистые мапперы и публичные типы backend-слоя.
- `src/runtime/` — универсальный слой управления процессом telemt (`TelemtRuntime`: systemd / external / none) и capability-модель для UI.
- `src/service.rs` — реализация вызовов `systemctl` и `journalctl` для режима `systemd`.
- `src/link.rs` — генерация секрета и `tg://proxy`-ссылки.
- `src/bot/handlers.rs` — сборка схемы обработчиков.
- `src/bot/handlers/commands/mod.rs` — slash-команды как точки входа в основные разделы и сценарии бота.
- `src/bot/handlers/callbacks/mod.rs` — inline callbacks и wizard-навигация.
- `src/bot/handlers/actions/service.rs` — orchestration для service panel, connections summary и service actions до передачи данных в presentation.
- `src/bot/handlers/actions/users.rs` — orchestration пользовательской карточки, lookup-сценариев, auto-import существующего `telemt`-пользователя и изменения live-лимитов.
- `src/bot/handlers/actions/access.rs` — approve/invite flow, выдача ссылок и правила auto-import/повторного invite для existing user.
- `src/bot/handlers/actions/broadcast.rs` — рассылка approved-пользователям и итоговая сводка по доставке.
- `src/bot/handlers/menu.rs` — текстовый ввод для активного wizard-state.
- `src/bot/keyboards.rs` — inline-клавиатуры.
- `src/bot/handlers/screens/mod.rs` — общие утилиты рендеринга экранов (`upsert_screen`, `page_bounds`, `compact_line`).
- `src/bot/handlers/screens/home.rs` — стартовые экраны админа и пользователя, гайд.
- `src/bot/handlers/screens/tokens.rs` — список токенов, карточка токена, подтверждение отзыва.
- `src/bot/handlers/screens/users.rs` — список пользователей, карточка пользователя, QR, подтверждение удаления.
- `src/bot/handlers/screens/pending.rs` — список заявок и карточка pending-заявки.
- `src/bot/handlers/screens/stats.rs` — сводка статистики и live-диагностика.
- `src/bot/handlers/screens/service.rs` — service panel и подтверждение действий над сервисом.
- `src/bot/handlers/screens/connections.rs` — экран top-пользователей по соединениям/трафику.
- `src/bot/handlers/screens/groups.rs` — меню групп и карточка группы.

Связанные документы:

- `docs/adr/README.md` — индекс ADR и точка входа в принятые решения.
- `docs/adr/001-telemt-api-backend.md` — почему выбран `API-first + fallback`.
- `docs/adr/002-telemt-api-security-and-rollout.md` — security-границы и стратегия rollout.
- `docs/runbook.md` — практическая эксплуатация и проверка rollout.

Принципы:

- Telegram/UI-логика не должна утекать в `db`, `service`, `telemt_cfg` и HTTP-клиент `telemt`.
- Инфраструктурные модули не должны возвращать готовые русские UI-строки, если вместо этого можно вернуть структурированный результат.
- Доменные операции лучше переиспользовать из общих функций, а не дублировать в командах и callbacks.
- Новые UX-сценарии желательно строить как `slash -> wizard/inline`, а не как роутинг по тексту сообщений.
- `/start` остаётся универсальной точкой входа: обычный user flow, invite-token deep link и admin deep link на конкретный экран/сущность.
- Для существующего пользователя `tg_<id>`, уже присутствующего в `telemt API`, user flow может автоматически создать локальную approved-запись без отдельного ручного импорта.
- Источником истины для wizard-state должна оставаться SQLite, чтобы сценарии корректно восстанавливались после рестарта и TTL применялся единообразно.
- Блокирующие системные команды не должны выполняться напрямую в async Telegram flow.
- Блокирующий файловый I/O (`telemt.toml`, self-update unpack/copy/rename) не должен выполняться напрямую в async Telegram flow; для этого используется явный offload через `spawn_blocking`.
- Фоновый мониторинг не должен дублировать UI-логику: он получает только структурированные snapshots и решает, слать ли уведомление.
- Редактируемые пользовательские тексты остаются на уровне конфигурации `[bot_messages]` и env overlay, без отдельного DB/UI-редактора.

Границы слоёв:

- `src/db/*.rs` возвращают данные и доменные состояния, а не готовую разметку экранов.
- `src/bot/handlers/screens/` — presentation-слой экранов бота (home, tokens, users, pending, stats, service, connections, groups) через `screens/mod.rs`;
- `src/bot/keyboards.rs` отвечает за inline-клавиатуры.
- orchestration уровня “БД + telemt backend + Telegram-ответ” лучше держать в action/use-case функциях, а не размазывать по callback/router-коду.
- service panel, connections summary и user card должны загружать данные через `actions/*`, а не собирать их напрямую внутри `screens`.
- parsing и применение wizard-значений для сроков/лимитов лучше держать рядом с use-case уровнем, а не размазывать по callback payload.
- `telemt_backend` должен оставаться единой внешней точкой выбора между control API и legacy file/systemd path; детали HTTP-клиента, DTO и mapping должны жить во внутренних подмодулях backend-слоя, а не утекать в handlers.
- `monitor` использует только `BotState` и структурированные ответы `telemt_backend`; он не должен напрямую читать БД-схему `telemt` или строить UI-экраны.

## Observability-стек

Проект предоставляет опциональный compose-стек (`deploy/compose/docker-compose.observability.yml`) с Prometheus и Grafana:

- **Prometheus** скрейпит `/metrics` от `telemt` (ожидается endpoint на порту control API или отдельном метрик-порту).
- **Grafana** поднимается с преднастроенным datasource и provisioned дашбордом `Telemt Overview` (uptime, connections, traffic, top users, handshake errors).
- **Сеть**: все сервисы (`telemt`, `telemt-admin`, `prometheus`, `grafana`) находятся в одной Docker bridge-сети `telemt-net`; Prometheus достаёт до `telemt` по DNS-имени.

Security:
- Если `/metrics` защищён `whitelist`, в `telemt.toml` нужно разрешить Docker-подсеть (обычно `172.16.0.0/12` или диапазон сети `telemt-net`).
- Если `/metrics` требует `auth_header`, токен прописывается в `config/prometheus.yml` (см. закомментированный блок `authorization`).
- Grafana по умолчанию создаёт администратора с credentials из `.env`; на production замените дефолтный пароль.

Новые runtime-возможности:

- `telemt_backend` умеет получать live-данные пользователя из `GET /v1/users/{username}` и менять лимиты через `PATCH /v1/users/{username}`;
- service panel показывает не только systemd status, но и runtime snapshots, нагрузку и top users;
- user card совмещает локальные sync-метаданные SQLite, `invite_token_id`, live-данные из control API и простые сигналы аномальной активности;
- admin stats используют не только SQLite-агрегаты, но и live summary/top users из `telemt API`, если они доступны;
- фоновые alert-ы управляются секцией `[notifications]` и используют polling `monitor.rs`.

База данных:

- схема обновляется автоматически при старте через `Db::migrate()`;
- текущий миграционный подход подходит для мягких изменений:
  - новые таблицы;
  - новые колонки;
  - новые индексы;
- сложные преобразования схемы лучше оформлять отдельно и явно.
- переходы состояний регистрации и consume invite-токена должны обновляться условными запросами, чтобы конкурентные действия не затирали друг друга.
- `invite_token_id` остаётся частью локальной registration-модели и используется как источник invite в карточках, pending-заявках и admin-уведомлениях.
- `user_groups.expires_at` хранится в unix-time и выступает общим сроком группы, который затем массово раскатывается на участников через control API.

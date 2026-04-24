# Пример Docker Compose для `telemt-admin`

Подготовка рабочей директории:

```bash
mkdir -p telemt-admin-docker/config telemt-admin-docker/data
cd telemt-admin-docker
curl -fsSLo docker-compose.yml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/docker-compose.telemt-admin.example.yml
curl -fsSLo .env.example https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/.env.example
curl -fsSLo config/telemt-admin.toml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/docker/telemt-admin.docker.toml.example
cp .env.example .env
```

После этого:

1. Отредактируйте `config/telemt-admin.toml`: `bot_token`, `admin_ids`, при необходимости `telemt_api.auth_header` и `base_url`. При необходимости задайте в `.env` `TELEMT_ADMIN_VERSION` (по умолчанию в образ уходит тег `latest`). Если из контейнера недоступен `api.telegram.org`, задайте `TELEMT_ADMIN__BOT_USERNAME` (username без `@`).
2. Внутри контейнера конфиг монтируется как `/etc/telemt-admin/telemt-admin.toml`.
3. Режим `external` и метка `docker` для UI задаются в `docker-compose.yml` (`TELEMT_ADMIN__RUNTIME__MODE`, `TELEMT_ADMIN__RUNTIME__LABEL`), переопределять в TOML не требуется.
4. Для контейнерного API-only сценария оставляйте `allow_file_fallback = false`. Тогда `telemt-admin` не пытается писать `telemt.toml`, и отдельный RW mount для него не нужен.
5. Экран **Top пользователей** работает даже без `runtime_edge_enabled=true` в telemt — бот автоматически использует fallback на список пользователей. Для нативной сводки с разбивкой ME/Direct включите `runtime_edge_enabled = true` в секции `[server.api]` конфига telemt.
6. Если хотите использовать конфигурируемые тексты бота, добавьте секцию `[bot_messages]` или env `TELEMT_ADMIN__BOT_MESSAGES__*` прямо в TOML / `.env`.
7. Уровень логов — переменная `RUST_LOG` в `.env` (например `info` или `debug` для отладки).

Минимальный пример `config/telemt-admin.toml`:

```toml
bot_token = "ВАШ_ТОКЕН_БОТА"
admin_ids = [123456789]

db_path = "/var/lib/telemt-admin/state.db"
telemt_config_path = "/etc/telemt.toml"

[telemt_api]
enabled = true
base_url = "http://telemt:9091"
auth_header = "Bearer ..."
timeout_ms = 5000
allow_file_fallback = false
prefer_api_links = true

[bot_messages]
start_without_invite = "Привет. Бот работает в закрытом режиме."
user_link_template = "Ваша ссылка:\n\n{link}"
access_approved_template = "Доступ готов.\n\n{link}"
```

Запуск:

```bash
docker compose pull
docker compose up -d
docker compose logs -f telemt-admin
```

Если контейнер постоянно перезапускается, смотрите логи командой выше. Частые причины: не задан `bot_token`, неверный тег в `TELEMT_ADMIN_VERSION`, недоступен Telegram API из сети контейнера.

Обновление:

```bash
docker compose pull
docker compose up -d
docker image prune -f
```

`telemt-admin` и `telemt` должны находиться в одной пользовательской сети Docker, если `telemt_api.base_url` указывает на имя контейнера. Для других схем используйте reachable host/IP, `host` network или `extra_hosts` по необходимости.

Если позже потребуется legacy fallback с записью в `telemt.toml`, это уже отдельный write-path: нужно явно смонтировать общий файл/том в контейнер `telemt-admin` c правом записи и понимать, что текущий пример compose этого не делает.

## Полный стек с observability (Prometheus + Grafana)

Файл `docker-compose.observability.yml` поднимает `telemt`, `telemt-admin`, Prometheus и Grafana в одной сети.

Подготовка:

```bash
mkdir -p telemt-observability/config telemt-observability/data
cd telemt-observability

curl -fsSLo docker-compose.yml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/docker-compose.observability.yml
curl -fsSLo .env.example https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/.env.example
curl -fsSLo config/telemt-admin.toml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/docker/telemt-admin.docker.toml.example

# Создайте минимальный config/telemt.toml для telemt
# (убедитесь, что server.api.listen = "0.0.0.0:9091",
#  и whitelist включает Docker-подсеть, например ["172.16.0.0/12", "127.0.0.1/32"])

cp .env.example .env
```

Запуск:

```bash
docker compose -f docker-compose.observability.yml up -d
```

После старта:
- Prometheus: http://localhost:9090 (или `${PROMETHEUS_PORT}`)
- Grafana: http://localhost:3000 (или `${GRAFANA_PORT}`), логин/пароль из `.env`
- Дашборд "Telemt Overview" уже импортирован через provisioning.

### Security note для /metrics

Если `telemt` отдаёт `/metrics` на том же порту, что и control API, и требует `auth_header` или `whitelist`:

- **Whitelist**: убедитесь, что `server.api.whitelist` в `telemt.toml` включает IP-адреса из Docker-сети. Prometheus скрейпит из контейнера, поэтому адрес источника будет из подсети `telemt-net`.
- **Auth**: если `/metrics` защищён Bearer-токеном, раскомментируйте блок `authorization:` в `config/prometheus.yml` и укажите токен.

Если `telemt` поддерживает отдельный порт или конфигурацию для `/metrics` без auth (например, `metrics_listen`), рекомендуется использовать его только внутри Docker-сети и не публиковать наружу.

### Про имена метрик в дашборде

Дашборд `config/grafana/dashboards/telemt-dashboard.json` использует ожидаемые имена метрик на основе полей control API (`uptime_seconds`, `connections_total`, `current_connections`, `handshake_timeouts_total`, `total_octets` и др.). Если `telemt` экспортирует метрики под другими именами, отредактируйте queries в дашборде после первого импорта.

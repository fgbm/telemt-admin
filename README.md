<p align="center">
  <img src="logo.png" alt="telemt-admin logo" width="800">
</p>

<h1 align="center">telemt-admin</h1>

**telemt-admin** — это Telegram-бот для автоматизации управления пользователями [telemt](https://github.com/telemt/telemt) (MTProto прокси-сервера). Проект создан для системных администраторов, которым нужно делегировать процесс выдачи доступа, избавившись от ручного редактирования конфигурационных файлов.

Поддерживает **систему пригласительных токенов** с двумя режимами работы:

- **Ручной (Manual):** пользователь подает заявку, админ подтверждает.
- **Автоматический (Auto-approve):** пользователь мгновенно получает доступ без участия админа.

**Семантика invite-токена:** при создании токена задаётся **срок действия ссылки** (в днях) и **лимит активаций** (сколько раз по ссылке можно зарегистрироваться). Это **не** срок подписки и **не** автоматическое ограничение доступа пользователя в `telemt` после регистрации. Дату истечения доступа пользователя, квоты трафика и прочие лимиты настраивают **отдельно** в карточке пользователя в боте (через control API `telemt`).

## Содержание

- [Зачем это нужно?](#зачем-это-нужно)
- [Требования](#требования)
- [Быстрый старт (Linux)](#быстрый-старт-linux)
- [Быстрый старт (Docker)](#быстрый-старт-docker)
- [Ручная установка](#ручная-установка)
- [Установка как системного сервиса](#установка-как-системного-сервиса)
- [Как пользоваться](#как-пользоваться)
- [Конфигурация (telemt-admin.toml)](#конфигурация-telemt-admintoml)
- [Проверка после запуска](#проверка-после-запуска)
- [Обновление](#обновление)
- [Сборка из исходников](#сборка-из-исходников)
- [Архитектурные решения и runbook](#архитектурные-решения-и-runbook)
- [Troubleshooting](#troubleshooting)
- [CI/CD](#cicd)

## Зачем это нужно?

Традиционное управление пользователями в `telemt` требует прямого доступа к серверу и ручного изменения `/etc/telemt.toml`.
**telemt-admin** переносит этот процесс в Telegram:

- **Для пользователей:** простая регистрация через бота по пригласительному токену.
- **Для администраторов:** гибкое управление доступом (временные invite-ссылки и лимит их активаций; лимиты пользователей в `telemt` — отдельно), мгновенные уведомления и управление через кнопки.
- **Для сервера:** управление пользователями через control API `telemt` с fallback на legacy-конфиг и `systemd`.

Интерфейс бота построен по модели, близкой к `BotFather`:

- основные разделы открываются через slash-команды;
- действия внутри разделов выполняются через inline-кнопки и короткие wizard-сценарии;
- пошаговое состояние wizard сохраняется в SQLite, восстанавливается после рестарта процесса и может автоматически истекать по TTL.

## Требования

- Linux-сервер с `systemd`.
- `root`-доступ или `sudo`.
- Telegram-бот и токен от [@BotFather](https://t.me/BotFather).
- Telegram user ID администраторов (можно получить через `@userinfobot`).
- Публичный IPv4 сервера для `announce`.
- Домен для `tls_domain`.

## Быстрый старт (Linux)

Основной сценарий установки теперь - один bootstrap-скрипт, который:

- скачивает последние релизы `telemt` и `telemt-admin`;
- кладёт бинарники в `/usr/local/bin`;
- создаёт `telemt.toml`, `telemt-admin.toml`, `systemd` unit-файлы и Polkit-правило;
- включает и запускает `telemt.service` и `telemt-admin.service`.

Запуск из интерактивного терминала:

```bash
curl -fsSL https://raw.githubusercontent.com/fgbm/telemt-admin/master/scripts/install.sh | sudo bash
```

Установщик задаёт интерактивные вопросы и читает ответы из терминала. Если TTY недоступен, скрипт завершится с ошибкой вместо бесконечных повторных prompt'ов.

Для CI, automation или сессий без TTY сначала сохраните скрипт в файл и запустите его локально:

```bash
curl -fsSLo install.sh https://raw.githubusercontent.com/fgbm/telemt-admin/master/scripts/install.sh
sudo bash ./install.sh
```

Во время установки скрипт спросит только минимально необходимые значения:

- `bot_token`
- `admin_ids`
- `port` для `telemt` (по умолчанию `443`)
- `tls_domain`
- `announce` (публичный IPv4 сервера)

После завершения можно проверить статус:

```bash
sudo systemctl status telemt.service
sudo systemctl status telemt-admin.service
```

Скрипт генерирует минимальный `/etc/telemt.toml` такого вида:

```toml
[general]
use_middle_proxy = false

[general.modes]
classic = false
secure = false
tls = true

[server]
port = 443

[server.api]
enabled = true
listen = "127.0.0.1:9091"
whitelist = ["127.0.0.1/32", "::1/128"]
auth_header = "Bearer <generated-secret>"

[censorship]
tls_domain = "site.example"

[[server.listeners]]
ip = "0.0.0.0"
announce = "X.X.X.X"
```

## Быстрый старт (Docker)

Если `telemt-admin` должен работать в контейнере, а сам `telemt` уже доступен по сети, подготовьте отдельную директорию для compose-стека:

```bash
mkdir -p telemt-admin-docker/config telemt-admin-docker/data
cd telemt-admin-docker
curl -fsSLo docker-compose.yml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/docker-compose.telemt-admin.example.yml
curl -fsSLo .env.example https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/compose/.env.example
curl -fsSLo config/telemt-admin.toml https://raw.githubusercontent.com/fgbm/telemt-admin/master/deploy/docker/telemt-admin.docker.toml.example
cp .env.example .env
```

Дальше:

1. Укажите в `config/telemt-admin.toml` как минимум `bot_token` и `admin_ids`, при необходимости — `telemt_api.auth_header` для control API. При необходимости задайте `TELEMT_ADMIN_VERSION` в `.env` (по умолчанию `latest`). Если из контейнера недоступен `api.telegram.org`, добавьте в `.env` `TELEMT_ADMIN__BOT_USERNAME` (username без `@`), иначе ссылки на токены и deep link не соберутся.
2. Отредактируйте при необходимости `telemt_api.base_url` так, чтобы URL был достижим из сети контейнера.
3. Режим `external` и метка для UI задаются в `docker-compose.yml` через `TELEMT_ADMIN__RUNTIME__MODE` / `TELEMT_ADMIN__RUNTIME__LABEL`. Держите `allow_file_fallback = false`, если не монтируете `telemt.toml`.
4. Если хотите конфигурируемые тексты бота, добавьте секцию `[bot_messages]` в TOML или переменные `TELEMT_ADMIN__BOT_MESSAGES__*` в `.env`.
5. Пример Compose ориентирован на API-only path. Если нужен legacy fallback с записью в `telemt.toml`, потребуется отдельный RW mount/volume, которого в примере нет.
6. Запустите стек:

```bash
docker compose pull
docker compose up -d
docker compose logs -f telemt-admin
```

Минимальный профиль `config/telemt-admin.toml` для Docker:

```toml
bot_token = "ВАШ_ТОКЕН_БОТА"
admin_ids = [123456789]

db_path = "/var/lib/telemt-admin/state.db"
telemt_config_path = "/etc/telemt.toml"

[telemt_api]
enabled = true
base_url = "http://telemt:9091"
timeout_ms = 5000
allow_file_fallback = false
prefer_api_links = true

[bot_messages]
start_without_invite = "Привет. Бот работает в закрытом режиме."
user_link_template = "Ваша ссылка:\n\n{link}"
access_approved_template = "Доступ готов.\n\n{link}"
```

Если `telemt` живёт в другом compose-стеке или на хосте, используйте такой `telemt_api.base_url`, который реально достижим из сети контейнера `telemt-admin`.

## Ручная установка

Если хотите развернуть всё вручную без bootstrap-скрипта, ниже остаётся reference-сценарий.

### 1. Скачивание актуальной версии `telemt-admin`

```bash
curl -L -o telemt-admin.tar.gz https://github.com/fgbm/telemt-admin/releases/latest/download/telemt-admin-linux-x86_64.tar.gz && tar -xzf telemt-admin.tar.gz
```

### 2. Установка бинарного файла

```bash
sudo mv telemt-admin /usr/local/bin/
sudo chmod +x /usr/local/bin/telemt-admin
```

### 3. Минимальная конфигурация `telemt-admin`

Создайте файл `/etc/telemt-admin.toml`:

```toml
bot_token = "ВАШ_ТОКЕН_БОТА"
admin_ids = [123456789] # Ваш Telegram ID
telemt_config_path = "/etc/telemt.toml"
db_path = "/var/lib/telemt-admin/state.db"
service_name = "telemt.service"
users_page_size = 10

[security]
default_token_days = 14
max_token_days = 180
allow_auto_approve_tokens = true
wizard_state_ttl_seconds = 86400 # опционально: TTL wizard-state в секундах

[telemt_api]
enabled = true
base_url = "http://127.0.0.1:9091"
auth_header = "Bearer ВАШ_ТОЧНЫЙ_AUTH_HEADER"
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

> [!TIP]
> Параметр `bot_token` можно не указывать в конфиге, если переменная окружения `TELOXIDE_TOKEN` задана в окружении процесса.

## Установка как системного сервиса

Для надежной работы в фоновом режиме настройте `systemd`:

### 1. Создайте пользователя и директории

```bash
sudo useradd --system --home /var/lib/telemt-admin --shell /usr/sbin/nologin telemt-admin
sudo mkdir -p /var/lib/telemt-admin
sudo chown -R telemt-admin:telemt-admin /var/lib/telemt-admin
```

### 2. Создайте unit-файл `/etc/systemd/system/telemt-admin.service`

```ini
[Unit]
Description=telemt-admin Telegram Bot Service
After=network-online.target

[Service]
Type=simple
User=telemt-admin
Group=telemt-admin
WorkingDirectory=/var/lib/telemt-admin
ExecStart=/usr/local/bin/telemt-admin /etc/telemt-admin.toml
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

### 3. Настройте права доступа

Чтобы сервис мог управлять `telemt` и редактировать его конфиг без прав root:

#### А. Разрешите управление `telemt.service` через Polkit

Создайте файл `/etc/polkit-1/rules.d/50-telemt-admin.rules`:

```javascript
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.systemd1.manage-units" &&
        action.lookup("unit") == "telemt.service" &&
        subject.user == "telemt-admin") {
        return polkit.Result.YES;
    }
});
```

Этого правила достаточно для команд `systemctl restart` и
`systemctl kill -s HUP --kill-who=main telemt.service`, которые использует бот.

#### Б. Настройте права на конфиг telemt

```bash
# Создаем группу telemt если её нет
sudo groupadd -f telemt

# Добавляем пользователя бота в группу
sudo usermod -aG telemt telemt-admin

# Меняем группу владельца конфига и даем права на запись группе
sudo chown :telemt /etc/telemt.toml
sudo chmod 664 /etc/telemt.toml
```

При такой настройке у пользователя `telemt-admin` есть право **читать fallback-конфиг** и при необходимости использовать legacy-путь записи в `/etc/telemt.toml`. В штатном режиме новые установки используют control API `telemt`, поэтому прямое редактирование файла больше не является основным механизмом.

### 4. Запустите сервис

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now telemt-admin.service
```

## Как пользоваться

### Для пользователей

1. Получить **пригласительный токен** от администратора.
2. Найти бота и нажать `/start`.
3. Ввести токен (или перейти по ссылке вида `https://t.me/MyBot?start=TOKEN`).
4. В зависимости от типа токена:
    - **Auto:** Бот сразу пришлет ссылку на прокси.
    - **Manual:** Бот создаст заявку ("Ожидайте подтверждения"), и после одобрения админом пришлет ссылку.

Если пользователь уже существует в `telemt` как `tg_<telegram_user_id>`, бот может автоматически подхватить его в локальную БД в сценариях `/start`, `/link` и invite. Повторный переход такого пользователя по invite-ссылке не увеличивает `usage_count`.

### Для администраторов

#### Управление заявками

При поступлении новой заявки (по Manual-токену) вы получите сообщение с кнопками:

- **✅ Одобрить**: генерация секрета, создание пользователя через control API `telemt`, сохранение локальной записи в БД, отправка ссылки пользователю. При сбое API возможен fallback на legacy-конфиг и `systemd`.
- **❌ Отклонить**: заявка отклоняется, пользователь получает уведомление.

В разделе `Заявки` доступен страничный список: новые заявки находятся выше, после открытия карточки можно вернуться на ту же страницу.

#### Управление токенами

Основной способ работы с токенами:

- `/token` — открыть inline-меню управления invite-токенами.
- Параметры при создании токена (дни и лимит активаций) относятся **к ссылке-приглашению**, а не к сроку доступа пользователя в `telemt`; последний задаётся в карточке пользователя после регистрации.
- В меню доступны создание токена и переход к списку активных токенов.
- Список токенов открывается с пагинацией и сортировкой по сроку действия: выше те, что истекают раньше.
- В списке токенов доступен поиск по коду.
- Отзыв токена выполняется из карточки конкретного токена.
- При создании токена бот возвращает готовую ссылку вида `https://t.me/MyBot?start=TOKEN` и код токена в моноширинном формате.

#### Админский интерфейс

После `/start` у администратора открывается главный inline-экран вместо постоянной reply-клавиатуры.
Основные разделы доступны через slash-команды:

- `/start` — открыть главный экран.
- `/user` — открыть раздел пользователей.
- `/service` — открыть компактную inline-панель управления `telemt.service`, control API и текущей нагрузкой.
- `/token` — открыть inline-меню invite-токенов.
- `/link` — получить свою proxy-ссылку.
- `/help` — показать список команд.

В списках пользователей и токенов доступен поиск, а основные разделы админки построены по схеме: список -> карточка -> действие -> назад.

Выдача доступа новым пользователям выполняется через invite-токены. Раздел `/user` предназначен для управления уже существующими записями: найти пользователя, открыть карточку, посмотреть deep link и QR, изменить лимиты или удалить доступ. Для самого администратора выдача собственной ссылки остаётся доступной через `/link`.

Если backend-операция завершается ошибкой, администратор получает причину в отдельном отформатированном сообщении в чате.

Дополнительно в админке доступны:

- `📢 Рассылка` — отправка эксплуатационного сообщения всем пользователям со статусом `approved`;
- `📁 Группы` — создание группы, назначение пользователей, массовое отключение и управление общим сроком группы;
- `📥 Импорт из telemt` — ручной импорт существующего `tg_<id>` из control API в локальную БД.

В карточке пользователя доступны действия:

- `🔄 Обновить` — перечитывает карточку и live-данные пользователя из `telemt` API.
- `🪄 Deep link` — отправляет deep link для быстрого открытия карточки пользователя через `/start`.
- `🔗 Данные + QR` — отправляет proxy-ссылку и QR-код для ручной пересылки пользователю.
- `TCP лимит` / `IP лимит` / `Квота` / `Истекает` — запускают короткий wizard для изменения лимитов пользователя через control API `telemt`.
- `⛔ Удалить пользователя` — удаляет пользователя из конфигурации `telemt` и деактивирует запись в БД.
- `⬅️ Назад к списку` — возвращает к той же странице пагинации.

Карточка пользователя показывает не только локальную запись из SQLite, но и live-данные из `telemt`, если control API доступен:

- текущие соединения;
- active/recent unique IPs;
- трафик;
- действующие лимиты;
- expiration и `user_ad_tag`;
- sync-метаданные локальной БД;
- `invite_token_id`, если пользователь пришёл по invite;
- простые сигналы аномальной активности, если live-поля выходят за лимиты.

В карточке токена доступны действия:

- `🔄 Обновить` — перечитывает карточку токена.
- `🔗 Ссылка` — отправляет готовую invite-ссылку для пересылки пользователю и краткую инструкцию, что делать дальше.
- `🗑 Отозвать токен` — деактивирует токен после подтверждения.
- `⬅️ Назад к списку` — возвращает к той же странице пагинации.

На сервисном экране рискованные действия (`Остановить`, `Перезапустить`) требуют подтверждения, а быстрые (`Обновить`, `Reload`) выполняются сразу. Дополнительно доступны:

- runtime snapshot по control API;
- сводка нагрузки (`uptime`, total/bad connections, handshake timeouts);
- live connections (`total`, `ME`, `Direct`, active users);
- sync-health по SQLite (`degraded`, `control_api`, `legacy`, top sync error codes);
- отдельный экран `📈 Top пользователей` по соединениям и трафику.

`/service` и экран top users теперь рассчитаны на частичную деградацию control API: при падении отдельного endpoint бот не скрывает весь экран, а показывает локальные данные и причину ошибки именно в проблемном блоке.

Deep links для администраторов поддерживают не только invite-token payload, но и быстрый переход к:

- карточке пользователя;
- карточке токена;
- основным admin screens через `/start`.

**Основные команды:**

- `/start` — открыть главный экран.
- `/user` — открыть раздел пользователей.
- `/token` — открыть меню invite-токенов.
- `/service` — открыть панель управления сервисом.
- `/help` — показать справку.
- `/link` — получить свою proxy-ссылку; для администратора при отсутствии учётной записи доступ будет создан автоматически.

## Архитектурные решения и runbook

Для API-first интеграции с `telemt` дополнительно зафиксированы:

- `docs/adr/README.md` — индекс архитектурных решений;
- `docs/adr/001-telemt-api-backend.md` — выбор backend-модели `API-first + fallback`;
- `docs/adr/002-telemt-api-security-and-rollout.md` — security-правила и стратегия rollout;
- `docs/adr/003-runtime-agnostic-deployment.md` — режимы runtime (systemd / Docker / без unit);
- `docs/adr/004-config-sources-and-docker-defaults.md` — TOML + whitelist `TELEMT_ADMIN__*`, пример конфига в образе;
- `docs/runbook.md` — пошаговое включение, проверка и откат integration path;
- `docs/backlog.md` — список следующих задач, включая observability и поддержку `/metrics`.

## Конфигурация (telemt-admin.toml)

- `bot_token` — токен бота от @BotFather (опционально, если задан `TELEMT_ADMIN__BOT_TOKEN` или `TELOXIDE_TOKEN`).
- `bot_username` — username бота в Telegram **без** ведущего `@` (опционально). Нужен для ссылок на invite-токены и deep link, если до `api.telegram.org` нет исходящего доступа и запрос `getMe` не проходит; при указании имеет приоритет над ответом `getMe`.
- `admin_ids` — массив ID администраторов `[123, 456]` (обязательный).
- `telemt_config_path` — путь к `/etc/telemt.toml` (default: `/etc/telemt.toml`).
- `db_path` — путь к `state.db` (default: `/var/lib/telemt-admin/state.db`).
- `service_name` — имя systemd unit для telemt (default: `telemt.service`). Используется в режиме `runtime.mode = "systemd"` и как запасной вариант, если в `[runtime]` не задано своё `service_name`.
- `[runtime]` — как бот видит управление процессом telemt на машине (опционально; если секции нет, считается `mode = "systemd"` и unit из `service_name`):
  - `mode` — `systemd` | `external` | `none` (default при явной секции: `systemd`).
  - `service_name` — override имени unit для `systemd` (иначе — верхнеуровневый `service_name`).
  - `label` — подпись в UI для `external` (например `docker compose`).
- `users_page_size` — размер страницы списков в админском интерфейсе, включая пользователей, invite-токены и заявки (default: `10`).
- `[telemt_api]` — настройки control API `telemt`:
  - `enabled` — включить API-first backend.
  - `base_url` — базовый URL control API, например `http://127.0.0.1:9091`.
  - `auth_header` — точное значение заголовка `Authorization`, которое ожидает `telemt`.
  - `timeout_ms` — таймаут HTTP-клиента.
  - `allow_file_fallback` — разрешить fallback на legacy-конфиг и `systemd`, если API недоступен.
  - `prefer_api_links` — приоритетно использовать ссылки, которые вернул control API.
- `[web_proxy]` — WEB-прокси `telemt` (тип прокси WEB в Telegram Desktop 7.1+, `telemt` 3.5+):
  - `host` — публичный hostname WEB-прокси без схемы, порта и пути, например `lk.example.com`. Если задан, бот добавляет к каждому сообщению со ссылкой блок с `tg://webproxy?server=<host>&secret=<dd>…`; секрет берётся из обычной ссылки пользователя. Пусто — WEB-ссылки не выдаются.
  - `secret_mode` — `dd` (по умолчанию) или `plain`; должен совпадать с `secret_mode` профиля в `[[web.vhosts.profiles]]` `telemt`.
  - Бот **не** создаёт WEB-профили в `telemt`: без профиля у пользователя WEB-ссылка не заработает.
- `[webapp]` — встроенное веб-приложение Telegram Mini App (ADR 005):
  - `enabled` — запустить HTTP-сервер приложения (default: `false`).
  - `listen` — адрес сервера (default: `127.0.0.1:8090`); наружу публикуется только через HTTPS reverse proxy.
  - `public_url` — публичный HTTPS URL, например `https://app.example.com/`; при заданном URL бот ставит кнопку меню, открывающую приложение.
  - `init_data_max_age_secs` — максимальный возраст подписанного `initData`, от 60 до 86400 (default: `3600`); значение вне диапазона — ошибка конфигурации.
  - `menu_button_text` — текст кнопки меню (default: `Прокси`).
  - Пользователь видит статус доступа, обычную и WEB-ссылку (копирование, QR, «Подключить»), инструкцию и может отправить invite-токен боту. Администратор — заявки, пользователей (поиск, карточка, удаление), invite-токены (создание, отзыв) и статус `telemt`.
  - Пример reverse proxy (Caddy): `app.example.com { reverse_proxy 127.0.0.1:8090 }`.
- `[security]` — настройки безопасности invite-токенов (срок действия **ссылки** и лимит активаций; не путать с лимитами пользователя в `telemt`):
  - `default_token_days` — срок действия invite-ссылки по умолчанию, в днях (default: 14).
  - `max_token_days` — максимально допустимый срок действия invite-ссылки (default: 180).
  - `allow_auto_approve_tokens` — разрешить создание auto-approve токенов (default: `true`).
  - `wizard_state_ttl_seconds` — опциональный TTL для сохранённого wizard-state в секундах. Если не задан, состояние хранится до явного завершения или отмены.
- `[notifications]` — фоновые уведомления и мониторинг:
  - `enabled` — включить фоновый polling `telemt` API и уведомления.
  - `health_check_interval_secs` — интервал health-check и runtime polling в секундах.
  - `notify_on_health_change` — уведомлять о смене health/API availability/accepting state/ME readiness.
  - `notify_on_runtime_alerts` — уведомлять о `unhealthy upstream`, ошибках `ME self-test` и восстановлении после них.
  - `notify_on_new_request` — отправлять уведомления о новых manual-заявках.
- `[bot_messages]` — переопределяемые тексты пользовательского UX и рассылки:
  - `start_without_invite` — текст-заглушка для `/start` без invite-токена; если задан, бот не переводит пользователя сразу в wizard ввода токена.
  - `invite_manual_prompt` — текст первого запроса invite-токена.
  - `invite_followup_prompt` — текст после кнопки «Ввести invite-токен».
  - `user_link_template` — шаблон сообщения со ссылкой; поддерживает `{link}`.
  - `access_approved_template` — шаблон auto-approve; поддерживает `{link}`.
  - `web_link_template` — блок с WEB-ссылкой, который добавляется к сообщениям со ссылкой при заданном `[web_proxy] host`; поддерживает `{web_link}`.
  - `request_submitted` — сообщение после создания manual-заявки.
  - `request_pending` — сообщение, если заявка уже ожидает решения.
  - `request_rejected` — сообщение для отклонённой заявки.
  - `broadcast_prompt` — подсказка перед рассылкой; поддерживает `{audience}`.
  - `broadcast_cancelled` — сообщение при отмене рассылки пустым текстом.
  - `broadcast_summary_template` — итог рассылки; поддерживает `{ok}`, `{failed}`, `{total}`.

### Переменные окружения `TELEMT_ADMIN__*` (overlay)

После чтения TOML можно переопределить отдельные поля через **whitelist** переменных с префиксом `TELEMT_ADMIN__` (приоритет выше, чем значения из файла). Полный список и правила — в [`docs/adr/004-config-sources-and-docker-defaults.md`](docs/adr/004-config-sources-and-docker-defaults.md).

Примеры:

- `TELEMT_ADMIN__BOT_TOKEN` — токен бота (альтернатива полю в TOML; также поддерживается `TELOXIDE_TOKEN`, если `bot_token` не задан).
- `TELEMT_ADMIN__BOT_USERNAME` — username без `@` (альтернатива полю `bot_username` в TOML).
- `TELEMT_ADMIN__ADMIN_IDS` — список ID через запятую, например `123,456`.
- `TELEMT_ADMIN__TELEMT_API__BASE_URL` — URL control API.
- `TELEMT_ADMIN__RUNTIME__MODE` — `systemd`, `external` или `none`.
- `TELEMT_ADMIN__BOT_MESSAGES__START_WITHOUT_INVITE` — заглушка для `/start` без invite.
- `TELEMT_ADMIN__BOT_MESSAGES__USER_LINK_TEMPLATE` — шаблон сообщения со ссылкой.
- `TELEMT_ADMIN__BOT_MESSAGES__BROADCAST_SUMMARY_TEMPLATE` — шаблон итоговой сводки рассылки.

Секреты не следует вшивать в `Dockerfile` через `ENV`; используйте Compose `environment`, `env_file` или секреты оркестратора.

### Wizard-state и БД

- Пошаговое состояние wizard хранится в SQLite-таблице `bot_wizard_states`.
- Схема БД обновляется автоматически при старте бота.
- SQLite является источником истины для wizard-state, поэтому активный сценарий переживает рестарт процесса.
- Если задан `security.wizard_state_ttl_seconds`, просроченные wizard-state автоматически удаляются и не восстанавливаются после истечения TTL.

## Проверка после запуска

Проверьте, что сервис запустился и бот отвечает:

```bash
sudo systemctl status telemt-admin.service
sudo journalctl -u telemt-admin.service -n 50 --no-pager
```

Минимальный smoke-check:

1. Напишите боту `/start` с аккаунта администратора.
2. Убедитесь, что открывается главный inline-экран администратора.
3. Откройте `/token`, создайте токен через кнопки и активируйте его пользовательским аккаунтом.
4. Откройте `/user`, зайдите в карточку пользователя и убедитесь, что видны live-данные и кнопки изменения лимитов.
5. Откройте `/service` и убедитесь, что видны runtime snapshot, блок нагрузки и экран `Top пользователей`.

## Обновление

### Обновление telemt (при установке быстрым скриптом)

```bash
curl -fsSL https://github.com/telemt/telemt/releases/latest/download/telemt-$(uname -m)-linux-gnu.tar.gz | sudo tar -xz -C /usr/local/bin/ && sudo systemctl restart telemt.service
```

### Проверка и автообновление telemt-admin

Проверить наличие новой версии:

```bash
telemt-admin check-update
```

На Linux x86_64 доступно автообновление (требуются права на запись в директорию с бинарником):

```bash
sudo telemt-admin self-update
```

Если бинарник установлен в `/usr/local/bin/` и сервис запущен от пользователя `telemt-admin`, для автообновления потребуется выполнить команду от root. После успешного обновления перезапустите сервис:

```bash
sudo systemctl restart telemt-admin.service
```

На Windows и других платформах автообновление не поддерживается — используйте ручное скачивание из раздела «Быстрый старт».

### Обновление Docker-развёртывания

Если `telemt-admin` запущен через Docker Compose, обновление сводится к подтягиванию нового тега образа и перезапуску контейнера:

```bash
cd telemt-admin-docker
docker compose pull
docker compose up -d
docker compose logs --tail=100 telemt-admin
```

Если вы хотите обновляться не на `latest`, а на фиксированную версию, задайте в `.env`, например, `TELEMT_ADMIN_VERSION=0.1.15`, и затем снова выполните `docker compose pull && docker compose up -d`.

## Docker

В репозитории есть `Dockerfile` (образ только с бинарником, **без** systemd) и пример Compose: [`deploy/compose/docker-compose.telemt-admin.example.yml`](deploy/compose/docker-compose.telemt-admin.example.yml). Для Docker quick start также есть шаблон `.env`: [`deploy/compose/.env.example`](deploy/compose/.env.example). Кратко: [`deploy/compose/README.md`](deploy/compose/README.md).

При создании GitHub-тега версии `vX.Y.Z` workflow релиза публикует образ в `ghcr.io/<owner>/telemt-admin` с тегами `X.Y.Z`, `X.Y`, `X`, `sha-<commit>` и `latest`.

В образе поставляется пример конфигурации без секретов: **`/usr/share/doc/telemt-admin/docker-default.toml.example`** (исходник: [`deploy/docker/telemt-admin.docker.toml.example`](deploy/docker/telemt-admin.docker.toml.example)). Его можно сохранить как `config/telemt-admin.toml` в compose-директории или взять за основу иным способом. Docker-образ по умолчанию ожидает рабочий конфиг по пути **`/etc/telemt-admin/telemt-admin.toml`** и объявляет тома **`/etc/telemt-admin`** и **`/var/lib/telemt-admin`**. Процесс в контейнере запускается от **root**, чтобы не ловить рассинхрон uid/gid на смонтированном каталоге данных; граница безопасности — изоляция контейнера и политика хоста.

Рекомендуемый профиль для контейнера:

- режим `external` (в примере Compose задаётся через `TELEMT_ADMIN__RUNTIME__MODE`);
- `[telemt_api] enabled = true` и `base_url` на доступный с сети контейнера control API telemt;
- `allow_file_fallback = false`, если нет смонтированного `telemt.toml` и systemd;
- bind mount или том на `db_path` для SQLite;
- при необходимости — переопределения через `TELEMT_ADMIN__*` (см. раздел выше).

## Сборка из исходников

Если вы хотите собрать бота самостоятельно:

1. Установите Rust (Edition 2024).
2. Выполните сборку: `cargo build --release`.
3. Бинарный файл будет находиться в `target/release/telemt-admin`.

### CLI

- `telemt-admin --help` / `-h` — справка по аргументам.
- `telemt-admin --version` / `-V` — версия.
- `telemt-admin -c <FILE>` / `--config <FILE>` — путь к конфигу.
- `telemt-admin check-update` — проверить наличие новой версии.
- `telemt-admin self-update` — автообновление (только Linux x86_64).

Запуск с кастомным конфигом (оба варианта поддерживаются):

```bash
./target/release/telemt-admin /path/to/telemt-admin.toml
./target/release/telemt-admin --config /path/to/telemt-admin.toml
```

## Troubleshooting

- `Не задан bot_token...`  
  Укажите `bot_token` в конфиге, либо `TELEMT_ADMIN__BOT_TOKEN`, либо `TELOXIDE_TOKEN` в окружении процесса.

- Нет ссылки на invite-токен / «username бота неизвестен»  
  Укажите `bot_username` в `telemt-admin.toml` или `TELEMT_ADMIN__BOT_USERNAME` в окружении, если исходящий доступ к Telegram API для `getMe` отсутствует или у бота в профиле не задан публичный username.

- `telemt API error ...`  
  Проверьте, что в `telemt` включён `[server.api]`, bind доступен с машины бота, а `telemt_api.auth_header` в `telemt-admin.toml` в точности совпадает с `server.api.auth_header`.

- `Permission denied` при записи в `/etc/telemt.toml`  
  Это важно только для fallback-режима. Проверьте группу/права файла и что пользователь `telemt-admin` входит в нужную группу `telemt`.

- Не удаётся выполнить `/service restart` или автоматическое применение конфига  
  Проверьте правило Polkit для `org.freedesktop.systemd1.manage-units`, корректность `service_name` и что у пользователя сервиса есть доступ к `telemt.service`.

- Бот не отвечает на команды  
  Проверьте логи `journalctl -u telemt-admin.service` и валидность токена бота.

## CI/CD

Проект использует GitHub Actions для автоматической проверки кода и публикации релизов.

- основной CI на `push`/`pull_request` запускает `cargo check --locked` и `cargo clippy --all-targets -- -D warnings`;
- release workflow по тегу `vX.Y.Z` собирает артефакты под Linux и Windows, публикует Docker-образ в GHCR и формирует release notes через `git-cliff`.

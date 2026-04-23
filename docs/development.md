# Разработка

Локальная проверка после заметных изменений:

```bash
cargo test --locked
cargo check --locked
cargo clippy --all-targets -- -D warnings
```

Если менялись зависимости:

```bash
cargo check
```

CI/CD:

- `.github/workflows/ci.yml` проверяет `cargo test --locked`, `cargo check --locked`, `cargo clippy --all-targets -- -D warnings` и `cargo audit --deny warnings` (см. `.cargo/audit.toml`);
- `.github/workflows/release.yml` собирает релизы для Linux x86_64, Linux arm64 и Windows x86_64 по тегам `v*.*.*`; публикует multi-platform Docker-образ (linux/amd64, linux/arm64) в GHCR (`ghcr.io/<owner>/telemt-admin`);
- release workflow не дублирует `clippy`, а предполагает, что релизный тег создаётся поверх кода, уже прошедшего основной CI;
- changelog формируется через `git-cliff` и Conventional Commits.
- релизные артефакты публикуются вместе с файлами `.sha256` (SHA-256 checksum); self-update проверяет контрольную сумму перед установкой;
- `scripts/install.sh` ориентирован на Linux x86_64 (glibc) + `systemd` и скачивает release-артефакты из GitHub.
- контейнерная сборка: корневой `Dockerfile`, пример `deploy/compose/docker-compose.telemt-admin.example.yml`;
- overlay конфигурации через whitelist `TELEMT_ADMIN__*` (см. `docs/adr/004-config-sources-and-docker-defaults.md`).

Конвенции:

- предпочтительный стиль коммитов: `feat:`, `fix:`, `refactor:`, `docs:` и т.д.;
- для покрытого тестами кода, критичной доменной логики и bugfix-путей придерживаться поэтапного TDD: сначала тест, затем минимальная реализация, затем рефакторинг;
- не писать шумовые тесты без пользы и не тащить e2e туда, где достаточно unit/integration уровня;
- не добавлять зависимости без явной необходимости;
- не использовать `unwrap()` там, где ошибка может быть штатной или внешней;
- если меняются архитектурные границы или модульная структура, синхронно обновлять `docs/*` и `.cursor/rules/*` в той же серии изменений;
- если меняются инварианты интеграции с `telemt`, стратегия fallback, rollout или security-модель, обновлять соответствующий ADR в `docs/adr/` или добавлять новый;
- если меняется эксплуатационное поведение control API, синхронно обновлять `docs/runbook.md`;
- если меняются notifications, health/runtime alerts или фоновый polling, синхронно обновлять `README.md`, `docs/overview.md`, `docs/architecture.md` и `docs/runbook.md`;
- при изменениях bot UX проверять вручную wizard-flow, основные inline-экраны и восстановление wizard-state после рестарта.
- unit-тесты в первую очередь писать для чистых helper-функций, мапперов, парсеров callback payload и форматирования; integration-тесты — для SQLite-слоя и миграций.
- если меняются `telemt_cfg`, legacy fallback или self-update, синхронно проверять, что блокирующий файловый I/O остаётся вынесенным из async executor и что ограничения по atomic write/offload отражены в docs.

Навигация по документации:

- `docs/overview.md` — краткая целевая модель проекта;
- `docs/architecture.md` — модули и границы слоёв;
- `docs/adr/README.md` — индекс архитектурных решений;
- `docs/runbook.md` — rollout, проверка и откат API-first интеграции;
- `docs/backlog.md` — следующий слой задач для реализации и наблюдаемости.

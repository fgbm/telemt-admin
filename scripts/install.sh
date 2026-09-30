#!/usr/bin/env bash
set -euo pipefail

TELEMT_ADMIN_REPO_OWNER="${TELEMT_ADMIN_REPO_OWNER:-fgbm}"
TELEMT_ADMIN_REPO_NAME="${TELEMT_ADMIN_REPO_NAME:-telemt-admin}"
TELEMT_ADMIN_ASSET="${TELEMT_ADMIN_ASSET:-telemt-admin-linux-x86_64.tar.gz}"

TELEMT_BIN_DIR="${TELEMT_BIN_DIR:-/usr/local/bin}"
TELEMT_ADMIN_BIN_DIR="${TELEMT_ADMIN_BIN_DIR:-/usr/local/bin}"
TELEMT_CONFIG_PATH="${TELEMT_CONFIG_PATH:-/etc/telemt.toml}"
TELEMT_ADMIN_CONFIG_PATH="${TELEMT_ADMIN_CONFIG_PATH:-/etc/telemt-admin.toml}"
TELEMT_WORK_DIR="${TELEMT_WORK_DIR:-/opt/telemt}"
TELEMT_ADMIN_STATE_DIR="${TELEMT_ADMIN_STATE_DIR:-/var/lib/telemt-admin}"
TELEMT_SERVICE_NAME="${TELEMT_SERVICE_NAME:-telemt.service}"
TELEMT_ADMIN_SERVICE_NAME="${TELEMT_ADMIN_SERVICE_NAME:-telemt-admin.service}"
TELEMT_USER="${TELEMT_USER:-telemt}"
TELEMT_ADMIN_USER="${TELEMT_ADMIN_USER:-telemt-admin}"
POLKIT_RULE_PATH="${POLKIT_RULE_PATH:-/etc/polkit-1/rules.d/50-telemt-admin.rules}"
PROMPT_INPUT_FD=0
PROMPT_INPUT_SOURCE="stdin"

if [ -t 1 ] && [ "${NO_COLOR:-}" = "" ] && [ "${TERM:-}" != "dumb" ]; then
    COLOR_BLUE='\033[1;34m'
    COLOR_CYAN='\033[1;36m'
    COLOR_GREEN='\033[1;32m'
    COLOR_YELLOW='\033[1;33m'
    COLOR_RED='\033[1;31m'
    COLOR_RESET='\033[0m'
else
    COLOR_BLUE=''
    COLOR_CYAN=''
    COLOR_GREEN=''
    COLOR_YELLOW=''
    COLOR_RED=''
    COLOR_RESET=''
fi

print_status() {
    local color="$1"
    local label="$2"
    shift 2
    printf '%b[%s]%b %s\n' "$color" "$label" "$COLOR_RESET" "$*"
}

info() {
    print_status "$COLOR_BLUE" "INFO" "$*"
}

step() {
    print_status "$COLOR_CYAN" "STEP" "$*"
}

success() {
    print_status "$COLOR_GREEN" " OK " "$*"
}

warn() {
    print_status "$COLOR_YELLOW" "WARN" "$*" >&2
}

fail() {
    print_status "$COLOR_RED" "ERROR" "$*" >&2
    exit 1
}

run_quiet() {
    local output

    if output="$("$@" 2>&1)"; then
        return 0
    fi

    if [ -n "$output" ]; then
        printf '%s\n' "$output" >&2
    fi
    fail "Команда завершилась с ошибкой: $*"
}

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

require_command() {
    command_exists "$1" || fail "Не найдена обязательная команда: $1"
}

escape_toml_basic_string() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    printf '%s' "$value"
}

backup_if_exists() {
    local target="$1"
    if [ -e "$target" ]; then
        local backup_path="${target}.bak.$(date +%Y%m%d%H%M%S)"
        cp -a "$target" "$backup_path"
        warn "Существующий файл сохранён в backup: $backup_path"
    fi
}

detect_nologin_shell() {
    if [ -x /usr/sbin/nologin ]; then
        printf '%s' /usr/sbin/nologin
        return 0
    fi

    if [ -x /usr/bin/nologin ]; then
        printf '%s' /usr/bin/nologin
        return 0
    fi

    printf '%s' /bin/false
}

ensure_root() {
    [ "$(id -u)" -eq 0 ] || fail "Запустите скрипт от root, например: curl -fsSL <URL> | sudo bash"
}

ensure_linux_x86_64_gnu() {
    [ "$(uname -s)" = "Linux" ] || fail "Поддерживается только Linux."
    [ "$(uname -m)" = "x86_64" ] || fail "MVP-установщик поддерживает только x86_64."
    require_command ldd
    if ldd --version 2>&1 | grep -qi musl; then
        fail "Обнаружен musl-based Linux. Для MVP поддерживается только glibc (gnu)."
    fi
}

ensure_systemd() {
    require_command systemctl
    [ -d /run/systemd/system ] || fail "systemd не обнаружен. Этот установщик рассчитан на systemd-based Linux."
}

setup_prompt_input() {
    if [ -t 0 ]; then
        PROMPT_INPUT_FD=0
        PROMPT_INPUT_SOURCE="stdin"
        return 0
    fi

    if exec 3</dev/tty 2>/dev/null; then
        PROMPT_INPUT_FD=3
        PROMPT_INPUT_SOURCE="/dev/tty"
        return 0
    fi

    fail "Интерактивный ввод недоступен. Запустите установщик из терминала с TTY. Для non-interactive среды сначала сохраните скрипт в файл и выполните его локально."
}

prompt_read_line() {
    local __result_var="$1"
    local __prompt_text="$2"
    # Имена локальных переменных с префиксом __, чтобы не затенять переменную
    # вызывающей функции (обычно `value`): иначе printf -v пишет в локальную копию.
    local __line

    printf '%s' "$__prompt_text" >&2
    if ! IFS= read -r -u "$PROMPT_INPUT_FD" __line; then
        fail "Не удалось прочитать ввод из ${PROMPT_INPUT_SOURCE}. Установщик требует интерактивный терминал."
    fi
    printf -v "$__result_var" '%s' "$__line"
}

validate_admin_ids() {
    local value="$1"
    printf '%s' "$value" | grep -Eq '^[0-9]+([[:space:]]*,[[:space:]]*[0-9]+)*$'
}

validate_port() {
    local value="$1"
    printf '%s' "$value" | grep -Eq '^[0-9]+$' || return 1
    [ "$value" -ge 1 ] && [ "$value" -le 65535 ]
}

validate_ipv4() {
    local value="$1"
    printf '%s' "$value" | grep -Eq '^([0-9]{1,3}\.){3}[0-9]{1,3}$' || return 1

    local old_ifs="$IFS"
    IFS='.'
    set -- $value
    IFS="$old_ifs"

    for octet in "$@"; do
        [ "$octet" -ge 0 ] 2>/dev/null || return 1
        [ "$octet" -le 255 ] 2>/dev/null || return 1
    done
}

is_port_free() {
    local port="$1"

    if command_exists ss; then
        ! ss -ltn | awk '{print $4}' | grep -Eq "(^|:)$port$"
        return
    fi

    if command_exists netstat; then
        ! netstat -ltn 2>/dev/null | awk '{print $4}' | grep -Eq "(^|:)$port$"
        return
    fi

    warn "Не удалось проверить занятость порта: ни ss, ни netstat не найдены."
}

prompt_nonempty() {
    local prompt_text="$1"
    local value=""
    while [ -z "$value" ]; do
        prompt_read_line value "${prompt_text}: "
        if [ -z "$value" ]; then
            warn "Значение не может быть пустым."
        fi
    done
    printf '%s' "$value"
}

prompt_admin_ids() {
    local value=""
    while true; do
        prompt_read_line value "Telegram admin ID или список через запятую: "
        if validate_admin_ids "$value"; then
            printf '%s' "$value"
            return 0
        fi
        warn "Ожидается один ID или список чисел через запятую."
    done
}

prompt_port() {
    local value=""
    while true; do
        prompt_read_line value "Порт для telemt [443]: "
        value="${value:-443}"
        if ! validate_port "$value"; then
            warn "Введите корректный TCP-порт в диапазоне 1..65535."
            continue
        fi
        if ! is_port_free "$value"; then
            warn "Порт $value уже занят. Выберите другой."
            continue
        fi
        printf '%s' "$value"
        return 0
    done
}

prompt_tls_domain() {
    prompt_nonempty "Домен для tls_domain (например, site.example)"
}

prompt_announce() {
    local value=""
    while true; do
        prompt_read_line value "Публичный IPv4 для announce: "
        if validate_ipv4 "$value"; then
            printf '%s' "$value"
            return 0
        fi
        warn "Ожидается корректный IPv4-адрес."
    done
}

csv_to_toml_int_array() {
    printf '%s' "$1" | awk -F',' '
        {
            for (i = 1; i <= NF; i++) {
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", $i)
                parts[++count] = $i
            }
        }
        END {
            for (i = 1; i <= count; i++) {
                printf "%s%s", parts[i], (i < count ? ", " : "")
            }
        }
    '
}

download_and_extract_tarball() {
    local url="$1"
    local destination_dir="$2"
    local asset_name="$3"
    local archive_path="$destination_dir/archive.tar.gz"

    step "Скачиваю ${asset_name}"
    info "$url"
    if [ -t 1 ] || [ -t 2 ]; then
        curl -fL --progress-bar "$url" -o "$archive_path"
    else
        curl -fsSL "$url" -o "$archive_path"
    fi
    step "Распаковываю ${asset_name}"
    tar -xzf "$archive_path" -C "$destination_dir"
}

ensure_group_exists() {
    local group_name="$1"
    if ! getent group "$group_name" >/dev/null 2>&1; then
        run_quiet groupadd --system "$group_name"
    fi
}

ensure_user_exists() {
    local user_name="$1"
    local home_dir="$2"
    local shell_path="$3"

    if id "$user_name" >/dev/null 2>&1; then
        return 0
    fi

    run_quiet useradd \
        --system \
        --gid "$user_name" \
        --home "$home_dir" \
        --create-home \
        --shell "$shell_path" \
        "$user_name"
}

main() {
    ensure_root
    ensure_linux_x86_64_gnu
    ensure_systemd

    require_command awk
    require_command curl
    require_command grep
    require_command install
    require_command tar
    require_command useradd
    require_command usermod
    require_command getent
    require_command groupadd
    require_command mktemp
    require_command od
    require_command tr

    info "Установка telemt + telemt-admin (Linux/systemd MVP)"
    info "Бинарники будут установлены в ${TELEMT_BIN_DIR%/}/telemt и ${TELEMT_ADMIN_BIN_DIR%/}/telemt-admin"
    setup_prompt_input

    local bot_token
    local admin_ids_csv
    local telemt_port
    local tls_domain
    local announce_ip
    local nologin_shell
    local admin_ids_toml
    local telemt_tmp
    local telemt_admin_tmp
    local telemt_url
    local telemt_admin_url
    local telemt_admin_config_dir
    local telemt_config_dir
    local escaped_bot_token
    local escaped_tls_domain
    local escaped_announce_ip
    local escaped_telemt_config_path
    local escaped_db_path
    local escaped_service_name
    local telemt_api_auth
    local escaped_telemt_api_auth
    local escaped_telemt_api_base_url

    bot_token="$(prompt_nonempty "Bot token от @BotFather")"
    admin_ids_csv="$(prompt_admin_ids)"
    telemt_port="$(prompt_port)"
    tls_domain="$(prompt_tls_domain)"
    announce_ip="$(prompt_announce)"

    admin_ids_toml="$(csv_to_toml_int_array "$admin_ids_csv")"
    nologin_shell="$(detect_nologin_shell)"
    telemt_url="https://github.com/telemt/telemt/releases/latest/download/telemt-$(uname -m)-linux-gnu.tar.gz"
    telemt_admin_url="https://github.com/${TELEMT_ADMIN_REPO_OWNER}/${TELEMT_ADMIN_REPO_NAME}/releases/latest/download/${TELEMT_ADMIN_ASSET}"
    telemt_tmp="$(mktemp -d)"
    telemt_admin_tmp="$(mktemp -d)"
    telemt_admin_config_dir="$(dirname "$TELEMT_ADMIN_CONFIG_PATH")"
    telemt_config_dir="$(dirname "$TELEMT_CONFIG_PATH")"

    trap "rm -rf '$telemt_tmp' '$telemt_admin_tmp'" EXIT

    escaped_bot_token="$(escape_toml_basic_string "$bot_token")"
    escaped_tls_domain="$(escape_toml_basic_string "$tls_domain")"
    escaped_announce_ip="$(escape_toml_basic_string "$announce_ip")"
    escaped_telemt_config_path="$(escape_toml_basic_string "$TELEMT_CONFIG_PATH")"
    escaped_db_path="$(escape_toml_basic_string "$TELEMT_ADMIN_STATE_DIR/state.db")"
    escaped_service_name="$(escape_toml_basic_string "$TELEMT_SERVICE_NAME")"
    telemt_api_auth="Bearer $(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
    escaped_telemt_api_auth="$(escape_toml_basic_string "$telemt_api_auth")"
    escaped_telemt_api_base_url="$(escape_toml_basic_string "http://127.0.0.1:9091")"

    step "Создаю системных пользователей и директории"
    ensure_group_exists "$TELEMT_USER"
    ensure_group_exists "$TELEMT_ADMIN_USER"
    ensure_user_exists "$TELEMT_USER" "$TELEMT_WORK_DIR" "$nologin_shell"
    ensure_user_exists "$TELEMT_ADMIN_USER" "$TELEMT_ADMIN_STATE_DIR" "$nologin_shell"
    run_quiet usermod -aG "$TELEMT_USER" "$TELEMT_ADMIN_USER"

    mkdir -p "$TELEMT_BIN_DIR" "$TELEMT_ADMIN_BIN_DIR" "$TELEMT_WORK_DIR" "$TELEMT_ADMIN_STATE_DIR" "$telemt_config_dir" "$telemt_admin_config_dir"
    chown "$TELEMT_USER:$TELEMT_USER" "$TELEMT_WORK_DIR"
    chown "$TELEMT_ADMIN_USER:$TELEMT_ADMIN_USER" "$TELEMT_ADMIN_STATE_DIR"

    download_and_extract_tarball "$telemt_url" "$telemt_tmp" "telemt"
    [ -f "$telemt_tmp/telemt" ] || fail "В архиве telemt не найден бинарник telemt."
    step "Устанавливаю бинарник telemt"
    install -m 0755 "$telemt_tmp/telemt" "$TELEMT_BIN_DIR/telemt"

    download_and_extract_tarball "$telemt_admin_url" "$telemt_admin_tmp" "telemt-admin"
    [ -f "$telemt_admin_tmp/telemt-admin" ] || fail "В архиве telemt-admin не найден бинарник telemt-admin."
    step "Устанавливаю бинарник telemt-admin"
    install -m 0755 "$telemt_admin_tmp/telemt-admin" "$TELEMT_ADMIN_BIN_DIR/telemt-admin"

    step "Генерирую конфиг telemt"
    backup_if_exists "$TELEMT_CONFIG_PATH"
    cat >"$TELEMT_CONFIG_PATH" <<EOF
# Generated by telemt-admin installer.

[general]
use_middle_proxy = false

[general.modes]
classic = false
secure = false
tls = true

[server]
port = ${telemt_port}

[server.api]
enabled = true
listen = "127.0.0.1:9091"
whitelist = ["127.0.0.1/32", "::1/128"]
auth_header = "${escaped_telemt_api_auth}"

[censorship]
tls_domain = "${escaped_tls_domain}"

[[server.listeners]]
ip = "0.0.0.0"
announce = "${escaped_announce_ip}"
EOF
    chown "$TELEMT_USER:$TELEMT_USER" "$TELEMT_CONFIG_PATH"
    chmod 0664 "$TELEMT_CONFIG_PATH"

    step "Генерирую конфиг telemt-admin"
    backup_if_exists "$TELEMT_ADMIN_CONFIG_PATH"
    cat >"$TELEMT_ADMIN_CONFIG_PATH" <<EOF
# Generated by telemt-admin installer.
bot_token = "${escaped_bot_token}"
admin_ids = [${admin_ids_toml}]
telemt_config_path = "${escaped_telemt_config_path}"
db_path = "${escaped_db_path}"
service_name = "${escaped_service_name}"
users_page_size = 10

[security]
default_token_days = 14
max_token_days = 180
allow_auto_approve_tokens = true
wizard_state_ttl_seconds = 86400

[telemt_api]
enabled = true
base_url = "${escaped_telemt_api_base_url}"
auth_header = "${escaped_telemt_api_auth}"
timeout_ms = 5000
allow_file_fallback = true
prefer_api_links = true
EOF
    chown "$TELEMT_ADMIN_USER:$TELEMT_ADMIN_USER" "$TELEMT_ADMIN_CONFIG_PATH"
    chmod 0600 "$TELEMT_ADMIN_CONFIG_PATH"

    step "Создаю unit-файл ${TELEMT_SERVICE_NAME}"
    backup_if_exists "/etc/systemd/system/${TELEMT_SERVICE_NAME}"
    cat >"/etc/systemd/system/${TELEMT_SERVICE_NAME}" <<EOF
[Unit]
Description=Telemt
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${TELEMT_USER}
Group=${TELEMT_USER}
WorkingDirectory=${TELEMT_WORK_DIR}
ExecStart=${TELEMT_BIN_DIR}/telemt ${TELEMT_CONFIG_PATH}
Restart=on-failure
LimitNOFILE=65536
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF

    step "Создаю unit-файл ${TELEMT_ADMIN_SERVICE_NAME}"
    backup_if_exists "/etc/systemd/system/${TELEMT_ADMIN_SERVICE_NAME}"
    cat >"/etc/systemd/system/${TELEMT_ADMIN_SERVICE_NAME}" <<EOF
[Unit]
Description=telemt-admin Telegram Bot Service
After=network-online.target ${TELEMT_SERVICE_NAME}
Wants=network-online.target

[Service]
Type=simple
User=${TELEMT_ADMIN_USER}
Group=${TELEMT_ADMIN_USER}
SupplementaryGroups=${TELEMT_USER}
WorkingDirectory=${TELEMT_ADMIN_STATE_DIR}
ExecStart=${TELEMT_ADMIN_BIN_DIR}/telemt-admin ${TELEMT_ADMIN_CONFIG_PATH}
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

    step "Настраиваю Polkit для ${TELEMT_ADMIN_USER}"
    mkdir -p "$(dirname "$POLKIT_RULE_PATH")"
    backup_if_exists "$POLKIT_RULE_PATH"
    cat >"$POLKIT_RULE_PATH" <<EOF
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.systemd1.manage-units" &&
        action.lookup("unit") == "${TELEMT_SERVICE_NAME}" &&
        subject.user == "${TELEMT_ADMIN_USER}") {
        return polkit.Result.YES;
    }
});
EOF
    chmod 0644 "$POLKIT_RULE_PATH"

    step "Перезагружаю systemd"
    run_quiet systemctl daemon-reload
    step "Запускаю ${TELEMT_SERVICE_NAME}"
    run_quiet systemctl enable --now "$TELEMT_SERVICE_NAME"
    step "Запускаю ${TELEMT_ADMIN_SERVICE_NAME}"
    run_quiet systemctl enable --now "$TELEMT_ADMIN_SERVICE_NAME"

    success "Установка завершена."
    info "telemt config: $TELEMT_CONFIG_PATH"
    info "telemt-admin config: $TELEMT_ADMIN_CONFIG_PATH"
    info "Проверка статуса:"
    info "  systemctl status $TELEMT_SERVICE_NAME"
    info "  systemctl status $TELEMT_ADMIN_SERVICE_NAME"
}

main "$@"

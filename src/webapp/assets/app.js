"use strict";

(function () {
  const tg = window.Telegram && window.Telegram.WebApp;
  const view = document.getElementById("view");
  const tabsNav = document.getElementById("tabs");
  const toastBox = document.getElementById("toast");
  const initData = tg ? tg.initData : "";

  let me = null;
  let summary = null;
  let currentTab = "access";
  let backHandler = null;
  // Номер навигации: ответ, пришедший после перехода на другой экран, не рисуется.
  let navSeq = 0;
  const nav = () => ++navSeq;
  const stale = (seq) => seq !== navSeq;

  // ---------- helpers ----------

  function el(tag, attrs, ...children) {
    const node = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs || {})) {
      if (value === null || value === undefined || value === false) continue;
      if (key === "class") node.className = value;
      else if (key.startsWith("on") && typeof value === "function") node.addEventListener(key.slice(2), value);
      else node.setAttribute(key, value === true ? "" : String(value));
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue;
      node.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
    return node;
  }

  let scrollTop = true;
  function render(...nodes) {
    view.replaceChildren(...nodes.filter((node) => node !== null && node !== undefined && node !== ""));
    if (scrollTop) window.scrollTo(0, 0);
    scrollTop = true;
  }

  /// Следующий render сохранит позицию прокрутки (перерисовка после действия).
  function keepScroll() {
    scrollTop = false;
  }

  let toastTimer = null;
  function toast(text) {
    toastBox.textContent = text;
    toastBox.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toastBox.hidden = true), 2600);
  }

  function haptic(type) {
    try {
      tg && tg.HapticFeedback && tg.HapticFeedback.notificationOccurred(type);
    } catch (_) { /* old clients */ }
  }

  let confirmOpen = false;
  function confirmAction(text) {
    return new Promise((resolve) => {
      if (confirmOpen) { resolve(false); return; }
      const native = tg && tg.showConfirm && (!tg.isVersionAtLeast || tg.isVersionAtLeast("6.2"));
      if (native) {
        confirmOpen = true;
        try {
          tg.showConfirm(text.slice(0, 250), (ok) => { confirmOpen = false; resolve(Boolean(ok)); });
        } catch (_) {
          // Попап уже открыт или параметры отвергнуты — не показываем второй диалог.
          confirmOpen = false;
          resolve(false);
        }
        return;
      }
      resolve(window.confirm(text));
    });
  }

  async function api(path, options) {
    const opts = options || {};
    const headers = { Authorization: "tma " + initData };
    if (opts.body !== undefined) headers["Content-Type"] = "application/json";
    let response;
    try {
      response = await fetch(path, {
        method: opts.method || "GET",
        headers,
        body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
      });
    } catch (_) {
      throw new Error("Нет связи с сервером, попробуйте позже");
    }
    if (opts.raw && response.ok) return response;
    let data = null;
    try { data = await response.json(); } catch (_) { data = null; }
    if (!response.ok) throw new Error((data && data.error) || "Ошибка " + response.status);
    return data;
  }

  function fmtDate(unix) {
    if (!unix) return "—";
    return new Date(unix * 1000).toLocaleString("ru-RU", {
      day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit",
    });
  }

  function fmtBytes(n) {
    if (!n) return "0 Б";
    const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
    let i = 0;
    let v = n;
    while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
    return (v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)) + " " + units[i];
  }

  function activityLine(a) {
    if (!a) return null;
    if (a.connections > 0) {
      return el("span", { class: "online" },
        el("span", { class: "dot" }), `соединений: ${a.connections} · IP: ${a.active_ips} · ${fmtBytes(a.total_octets)}`);
    }
    return el("span", { class: "muted" }, `не в сети · ${fmtBytes(a.total_octets)}`);
  }

  function who(item) {
    const parts = [];
    if (item.name) parts.push(item.name);
    if (item.username) parts.push("@" + item.username);
    return parts.length ? parts.join(" · ") : "id " + item.tg_user_id;
  }

  function legacyCopy(text) {
    const area = el("textarea", { readonly: true, class: "offscreen" });
    area.value = text;
    document.body.append(area);
    area.select();
    area.setSelectionRange(0, text.length);
    let ok = false;
    try { ok = document.execCommand("copy"); } catch (_) { ok = false; }
    area.remove();
    return ok;
  }

  async function copy(text) {
    let ok = false;
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        ok = true;
      }
    } catch (_) { ok = false; }
    if (!ok) ok = legacyCopy(text);
    if (ok) { toast("Скопировано"); haptic("success"); }
    else toast("Не удалось скопировать — нажмите на ссылку и выделите её вручную");
  }

  function openProxyLink(link) {
    // tg://proxy?… → https://t.me/proxy?… — Telegram откроет диалог добавления прокси.
    const url = link.replace(/^tg:\/\/(proxy|socks)\?/, "https://t.me/$1?");
    if (!url.startsWith("https://t.me/")) { copy(link); return; }
    if (tg && tg.openTelegramLink) tg.openTelegramLink(url);
    else window.location.href = url;
  }

  async function showQr(container, kind) {
    const old = container.querySelector("img.qr");
    if (old) { old.remove(); return; }
    if (container.dataset.loading) return;
    container.dataset.loading = "1";
    try {
      const response = await api("/api/me/qr?kind=" + encodeURIComponent(kind), { raw: true });
      const url = URL.createObjectURL(await response.blob());
      const img = el("img", { class: "qr", alt: "QR-код для подключения" });
      img.addEventListener("load", () => URL.revokeObjectURL(url), { once: true });
      img.addEventListener("error", () => URL.revokeObjectURL(url), { once: true });
      img.src = url;
      container.append(img);
    } catch (error) {
      toast(error.message);
    } finally {
      delete container.dataset.loading;
    }
  }

  function setBack(handler) {
    backHandler = handler;
    if (!tg || !tg.BackButton) return;
    if (handler) tg.BackButton.show();
    else tg.BackButton.hide();
  }

  function errorCard(error) {
    return el("div", { class: "card" }, el("p", { class: "danger" }, error.message || String(error)));
  }

  /// Если страница опустела (удалили последнее), вернуться на предыдущую.
  function emptyPage(data) {
    return data.items.length === 0 && data.page > 0;
  }

  const MODE_NAMES = { ControlApi: "control API", LegacyFile: "файл конфига (legacy)" };

  function fmtUptime(seconds) {
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes} мин`;
    const hours = Math.floor(minutes / 60);
    return hours >= 48 ? `${Math.floor(hours / 24)} д` : `${hours} ч`;
  }

  function pager(page, total, pageSize, onPage) {
    const pages = Math.max(1, Math.ceil(total / pageSize));
    if (pages <= 1) return null;
    return el("div", { class: "pager" },
      el("button", { class: "btn secondary", disabled: page <= 0, onclick: () => onPage(page - 1) }, "‹ Назад"),
      el("span", { class: "muted small" }, `${page + 1} / ${pages}`),
      el("button", { class: "btn secondary", disabled: page + 1 >= pages, onclick: () => onPage(page + 1) }, "Вперёд ›"));
  }

  // ---------- tabs ----------

  const TABS = [
    { id: "access", title: "Доступ", admin: false, show: showAccess },
    { id: "requests", title: "Заявки", admin: true, show: showRequests },
    { id: "users", title: "Пользователи", admin: true, show: showUsers },
    { id: "tokens", title: "Токены", admin: true, show: showTokens },
    { id: "status", title: "Статус", admin: true, show: showStatus },
  ];

  function drawTabs() {
    const visible = TABS.filter((tab) => !tab.admin || (me && me.is_admin));
    tabsNav.hidden = visible.length < 2;
    tabsNav.setAttribute("role", "tablist");
    tabsNav.replaceChildren(...visible.map((tab) =>
      el("button", {
        class: tab.id === currentTab ? "active" : "",
        role: "tab",
        "aria-selected": tab.id === currentTab ? "true" : "false",
        onclick: () => openTab(tab.id),
      }, tab.title)));
    const active = tabsNav.querySelector("button.active");
    if (active) active.scrollIntoView({ block: "nearest", inline: "center" });
  }

  function openTab(id) {
    nav();
    currentTab = id;
    setBack(null);
    drawTabs();
    const tab = TABS.find((t) => t.id === id) || TABS[0];
    tab.show();
  }

  // ---------- user ----------

  function linkBlock(title, link, kind, hint) {
    const box = el("div", {});
    box.append(
      el("h3", {}, title),
      hint ? el("p", { class: "muted small" }, hint) : null,
      el("div", { class: "link-box" }, link),
      el("div", { class: "row" },
        el("button", { class: "btn", onclick: () => copy(link) }, "Копировать"),
        kind === "native" ? el("button", { class: "btn secondary", onclick: () => openProxyLink(link) }, "Подключить") : null,
        el("button", { class: "btn secondary", onclick: () => showQr(box, kind) }, "QR-код")));
    return box;
  }

  function showAccess() {
    const card = el("div", { class: "card" });
    const hello = me.name ? `${me.name}` : "Здравствуйте";
    card.append(el("h2", {}, hello));

    if (me.access === "approved") {
      card.append(el("p", {}, el("span", { class: "badge ok" }, "Доступ открыт")));
      if (me.links) {
        card.append(linkBlock("Обычный прокси", me.links.native, "native",
          "Для телефона и компьютера. Нажмите «Подключить» и подтвердите добавление прокси."));
        if (me.links.web) {
          card.append(linkBlock("WEB-прокси", me.links.web, "web",
            "Маскируется под обычный сайт. Пока работает только в Telegram Desktop 7.1+."));
        }
      } else {
        card.append(el("p", { class: "danger" }, me.links_error || "Ссылка временно недоступна"));
      }
      render(card, instructionsCard());
      return;
    }

    if (me.access === "pending") {
      card.append(
        el("p", {}, el("span", { class: "badge warn" }, "Заявка на рассмотрении")),
        el("p", { class: "muted" }, "Администратор получил вашу заявку. Ссылка придёт в чат с ботом после одобрения."));
      render(card);
      return;
    }

    const statusText = {
      rejected: "Заявка отклонена",
      deleted: "Доступ закрыт",
      none: "Доступа пока нет",
    }[me.access] || "Доступа нет";
    card.append(
      el("p", {}, el("span", { class: "badge bad" }, statusText)),
      el("p", { class: "muted" }, "Введите пригласительный токен — бот проверит его и выдаст ссылку."));
    const input = el("input", { id: "invite-token", type: "text", placeholder: "Токен", autocomplete: "off", autocapitalize: "off", spellcheck: "false" });
    const submit = el("button", { class: "btn", onclick: () => {
      const token = input.value.trim();
      if (!token) { toast("Введите токен"); return; }
      if (!/^[A-Za-z0-9_-]{1,64}$/.test(token)) { toast("Токен содержит недопустимые символы"); return; }
      if (!me.bot_username) { toast("Отправьте боту: /start " + token); return; }
      const url = `https://t.me/${encodeURIComponent(me.bot_username)}?start=${encodeURIComponent(token)}`;
      if (tg && tg.openTelegramLink) tg.openTelegramLink(url);
      else window.location.href = url;
    } }, "Отправить боту");
    card.append(el("label", { for: "invite-token" }, "Пригласительный токен"), input, el("div", { class: "row" }, submit));
    render(card);
  }

  function instructionsCard() {
    const card = el("div", { class: "card" }, el("h2", {}, "Как подключить"));
    card.append(
      el("h3", {}, "Обычный прокси"),
      el("ol", { class: "steps" },
        el("li", {}, "Нажмите «Подключить» или откройте ссылку в Telegram."),
        el("li", {}, "Подтвердите «Подключить прокси»."),
        el("li", {}, "Если не соединяется — проверьте, не включён ли VPN.")));
    if (me.links && me.links.web) {
      card.append(
        el("h3", {}, "WEB-прокси (Telegram Desktop)"),
        el("ol", { class: "steps" },
          el("li", {}, "Настройки → Продвинутые → Тип соединения → Прокси."),
          el("li", {}, "Добавить прокси → тип WEB."),
          el("li", {}, "Сервер: " + (me.web_proxy_host || "из ссылки") + ", секрет — часть ссылки после secret="),
          el("li", {}, "Первое подключение может занять до минуты.")));
    }
    return card;
  }

  // ---------- admin: requests ----------

  async function showRequests(page, keep) {
    page = page || 0;
    const seq = nav();
    if (!keep) render(el("p", { class: "muted center" }, "Загрузка…"));
    let data;
    try { data = await api("/api/admin/requests?page=" + page); } catch (e) { if (!stale(seq)) render(errorCard(e)); return; }
    if (stale(seq)) return;
    if (emptyPage(data)) { showRequests(data.page - 1, keep); return; }
    const card = el("div", { class: "card" }, el("h2", {}, `Заявки (${data.total})`));
    if (!data.items.length) card.append(el("p", { class: "muted" }, "Новых заявок нет"));
    for (const item of data.items) {
      const row = el("div", { class: "list-item" },
        el("div", { class: "main" },
          el("div", { class: "title" }, who(item)),
          el("div", { class: "muted small" }, `id ${item.tg_user_id} · ${fmtDate(item.created_at)}`)),
        el("div", { class: "row" },
          el("button", { class: "btn", onclick: (ev) => decide(ev, item, "approve", page) }, "Одобрить"),
          el("button", { class: "btn danger", onclick: (ev) => decide(ev, item, "reject", page) }, "Отклонить")));
      card.append(row);
    }
    if (keep) keepScroll();
    render(card, pager(data.page, data.total, data.page_size, (p) => showRequests(p)));
  }

  async function decide(ev, item, action, page) {
    const verb = action === "approve" ? "Одобрить" : "Отклонить";
    const button = ev.currentTarget;
    const seq = navSeq;
    if (!(await confirmAction(`${verb} заявку: ${who(item)}?`))) return;
    button.disabled = true;
    try {
      const result = await api(`/api/admin/requests/${item.id}/${action}`, { method: "POST", body: {} });
      toast(result.message);
      haptic("success");
    } catch (e) {
      toast(e.message);
      haptic("error");
    }
    if (!stale(seq)) showRequests(page, true);
  }

  // ---------- admin: users ----------

  let usersQuery = "";
  let usersOnline = false;

  async function showUsers(page, keep) {
    page = page || 0;
    const seq = nav();
    const search = el("input", { type: "search", placeholder: "Поиск: имя, @username или id", "aria-label": "Поиск пользователей", value: usersQuery });
    search.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") { usersQuery = search.value.trim(); showUsers(0); }
    });
    const modes = el("div", { class: "segmented" },
      el("button", { class: usersOnline ? "" : "active", onclick: () => { usersOnline = false; showUsers(0); } }, "Все"),
      el("button", { class: usersOnline ? "active" : "", onclick: () => { usersOnline = true; usersQuery = ""; showUsers(0); } }, "Онлайн сейчас"));
    const searchCard = el("div", { class: "card" }, modes,
      usersOnline ? null : search,
      usersOnline ? null : el("div", { class: "row" },
        el("button", { class: "btn", onclick: () => { usersQuery = search.value.trim(); showUsers(0); } }, "Найти"),
        usersQuery ? el("button", { class: "btn secondary", onclick: () => { usersQuery = ""; showUsers(0); } }, "Сбросить") : null));
    if (!keep) render(searchCard, el("p", { class: "muted center" }, "Загрузка…"));
    const qs = usersOnline ? "online=true" : usersQuery ? "q=" + encodeURIComponent(usersQuery) : "page=" + page;
    let data;
    try { data = await api("/api/admin/users?" + qs); } catch (e) { if (!stale(seq)) render(searchCard, errorCard(e)); return; }
    if (stale(seq)) return;
    if (!usersOnline && !usersQuery && emptyPage(data)) { showUsers(data.page - 1, keep); return; }
    const title = usersOnline ? `Онлайн сейчас: ${data.total}` : usersQuery ? `Найдено: ${data.total}` : `Пользователи (${data.total})`;
    const list = el("div", { class: "card" }, el("h2", {}, title));
    if (!data.items.length) list.append(el("p", { class: "muted" }, usersOnline ? "Сейчас никто не подключён" : "Никого не найдено"));
    if (data.truncated) {
      list.append(el("p", { class: "muted small" }, usersOnline
        ? `Показаны первые ${data.items.length} из ${data.total}.`
        : `Показаны первые ${data.items.length} совпадений — уточните запрос.`));
    }
    for (const item of data.items) {
      // Карточка есть только у пользователей из БД бота (у неизвестных telemt-пользователей created_at = 0).
      const clickable = item.tg_user_id > 0 && item.created_at > 0;
      const open = () => showUserCard(item.tg_user_id, page);
      list.append(el("div", {
        class: clickable ? "list-item clickable" : "list-item",
        role: clickable ? "button" : null,
        tabindex: clickable ? 0 : null,
        onclick: clickable ? open : null,
        onkeydown: clickable ? (ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); open(); } } : null,
      },
        el("div", { class: "main" },
          el("div", { class: "title" }, item.created_at > 0 ? who(item) : (item.telemt_username || "—"),
            item.status === "unknown" ? el("span", { class: "badge warn" }, "нет в боте") : null),
          el("div", { class: "small" }, activityLine(item.activity)),
          el("div", { class: "muted small" }, `${item.telemt_username || "—"}` + (item.created_at ? ` · с ${fmtDate(item.created_at)}` : ""))),
        el("span", { class: "chevron muted" }, "›")));
    }
    if (keep) keepScroll();
    render(searchCard, list, usersQuery || usersOnline ? null : pager(data.page, data.total, data.page_size, (p) => showUsers(p)));
  }

  const SYNC_NOTES = {
    degraded_legacy_fallback: "Создан через запасной путь (запись в файл конфига telemt), когда API был недоступен. На работу доступа не влияет.",
  };

  function syncNote(code) {
    if (!code) return null;
    const text = SYNC_NOTES[code] || "Последняя синхронизация с telemt завершилась с ошибкой: " + code;
    return el("p", { class: "small muted" }, text);
  }

  async function showUserCard(tgUserId, backPage) {
    const seq = nav();
    setBack(() => { setBack(null); showUsers(backPage); });
    render(el("p", { class: "muted center" }, "Загрузка…"));
    let data;
    try { data = await api("/api/admin/users/" + tgUserId); } catch (e) { if (!stale(seq)) render(errorCard(e)); return; }
    if (stale(seq)) return;
    const u = data.user;
    const card = el("div", { class: "card" },
      el("h2", {}, who(u)),
      el("p", { class: "muted small" }, `Telegram id ${u.tg_user_id} · ${u.telemt_username || "—"} · заявка от ${fmtDate(u.created_at)}`),
      u.activity ? el("p", {}, activityLine(u.activity)) : null,
      syncNote(u.last_sync_error));
    if (data.links) {
      card.append(
        el("h3", {}, "Обычная ссылка"), el("div", { class: "link-box" }, data.links.native),
        el("div", { class: "row" }, el("button", { class: "btn secondary", onclick: () => copy(data.links.native) }, "Копировать")));
      if (data.links.web) {
        card.append(
          el("h3", {}, "WEB-ссылка"), el("div", { class: "link-box" }, data.links.web),
          el("div", { class: "row" }, el("button", { class: "btn secondary", onclick: () => copy(data.links.web) }, "Копировать")));
      }
    } else if (data.links_error) {
      card.append(el("p", { class: "danger" }, data.links_error));
    }
    card.append(el("div", { class: "row" },
      el("button", { class: "btn danger", onclick: async (ev) => {
        const button = ev.currentTarget;
        const cardSeq = navSeq;
        if (!(await confirmAction(`Удалить доступ пользователя ${who(u)}? Ссылка перестанет работать.`))) return;
        button.disabled = true;
        try {
          const result = await api("/api/admin/users/" + u.tg_user_id, { method: "DELETE" });
          toast(result.message);
          haptic("success");
          if (!stale(cardSeq)) { setBack(null); showUsers(backPage); }
        } catch (e) {
          toast(e.message);
          haptic("error");
          button.disabled = false;
        }
      } }, "Удалить доступ")));
    render(card);
  }

  // ---------- admin: tokens ----------

  async function showTokens(page) {
    page = page || 0;
    const seq = nav();
    render(el("p", { class: "muted center" }, "Загрузка…"));
    let data;
    try {
      summary = await api("/api/admin/summary");
      data = await api("/api/admin/tokens?page=" + page);
    } catch (e) { if (!stale(seq)) render(errorCard(e)); return; }
    if (stale(seq)) return;

    const days = el("input", { id: "token-days", type: "number", inputmode: "numeric", min: 1, max: summary.max_token_days, value: summary.default_token_days });
    const maxUsage = el("input", { id: "token-max-usage", type: "number", inputmode: "numeric", min: 1, placeholder: "без ограничения" });
    const auto = el("input", { type: "checkbox" });
    const result = el("div", {});
    const create = el("div", { class: "card" },
      el("h2", {}, "Новый токен"),
      el("label", { for: "token-days" }, `Срок действия, дней (до ${summary.max_token_days})`), days,
      el("label", { for: "token-max-usage" }, "Лимит активаций"), maxUsage,
      summary.allow_auto_approve_tokens ? el("label", { class: "check" }, auto, "Одобрять автоматически") : null,
      el("div", { class: "row" }, el("button", { class: "btn", onclick: async (ev) => {
        const button = ev.currentTarget;
        const daysValue = Number(days.value);
        if (!Number.isInteger(daysValue) || daysValue < 1 || daysValue > summary.max_token_days) {
          toast(`Срок действия — целое число от 1 до ${summary.max_token_days}`);
          return;
        }
        const usageValue = maxUsage.value.trim() === "" ? null : Number(maxUsage.value);
        if (usageValue !== null && (!Number.isInteger(usageValue) || usageValue < 1 || usageValue > 100000)) {
          toast("Лимит активаций — целое число от 1 до 100000 или пусто");
          return;
        }
        button.disabled = true;
        try {
          const token = await api("/api/admin/tokens", { method: "POST", body: {
            days: daysValue,
            max_usage: usageValue,
            auto_approve: auto.checked,
          } });
          haptic("success");
          const link = token.start_link || token.token;
          result.replaceChildren(
            el("h3", {}, "Токен создан"), el("div", { class: "link-box" }, link),
            el("div", { class: "row" }, el("button", { class: "btn secondary", onclick: () => copy(link) }, "Копировать")));
          showTokenList(list, 0);
        } catch (e) {
          toast(e.message);
          haptic("error");
        }
        button.disabled = false;
      } }, "Создать")),
      result);
    const list = el("div", {});
    render(create, list);
    renderTokenList(list, data);
  }

  async function showTokenList(container, page) {
    const seq = navSeq;
    try {
      const data = await api("/api/admin/tokens?page=" + page);
      if (stale(seq)) return;
      if (emptyPage(data)) { showTokenList(container, data.page - 1); return; }
      renderTokenList(container, data);
    } catch (e) { if (!stale(seq)) container.replaceChildren(errorCard(e)); }
  }

  function renderTokenList(container, data) {
    const card = el("div", { class: "card" }, el("h2", {}, `Активные токены (${data.total})`));
    if (!data.items.length) card.append(el("p", { class: "muted" }, "Активных токенов нет"));
    for (const t of data.items) {
      const usage = t.max_usage ? `${t.usage_count}/${t.max_usage}` : `${t.usage_count}`;
      const link = t.start_link || t.token;
      card.append(el("div", { class: "list-item" },
        el("div", { class: "main" },
          el("div", { class: "title" }, t.token, " ", t.auto_approve ? el("span", { class: "badge ok" }, "авто") : null),
          el("div", { class: "muted small" }, `до ${fmtDate(t.expires_at)} · активаций ${usage}`)),
        el("div", { class: "row" },
          el("button", { class: "btn secondary", onclick: () => copy(link) }, "Ссылка"),
          el("button", { class: "btn danger", onclick: async (ev) => {
            const button = ev.currentTarget;
            if (!(await confirmAction(`Отозвать токен ${t.token}?`))) return;
            button.disabled = true;
            try {
              const r = await api(`/api/admin/tokens/${t.id}/revoke`, { method: "POST", body: {} });
              toast(r.message);
              haptic("success");
              showTokenList(container, data.page);
            } catch (e) { toast(e.message); button.disabled = false; }
          } }, "Отозвать"))));
    }
    container.replaceChildren(card, pager(data.page, data.total, data.page_size, (p) => showTokenList(container, p)) || "");
  }

  // ---------- admin: status ----------

  async function showStatus() {
    const seq = nav();
    render(el("p", { class: "muted center" }, "Загрузка…"));
    try { summary = await api("/api/admin/summary"); } catch (e) { if (!stale(seq)) render(errorCard(e)); return; }
    if (stale(seq)) return;
    const s = summary;
    const stat = (value, label, hint) => el("div", { class: "stat" },
      el("div", { class: "value" }, value), el("div", { class: "label" }, label),
      hint ? el("div", { class: "hint" }, hint) : null);
    const online = s.online
      ? el("div", { class: "card" }, el("h2", {}, "Сейчас"),
          el("div", { class: "stats" },
            stat(s.online.active_users, "онлайн пользователей", "с открытыми соединениями"),
            stat(s.online.current_connections, "соединений сейчас"),
            stat(s.online.active_ips, "активных IP")),
          el("div", { class: "row" }, el("button", { class: "btn secondary", onclick: () => { usersOnline = true; usersQuery = ""; openTab("users"); } }, "Кто онлайн ›")))
      : null;
    const users = el("div", { class: "card" }, el("h2", {}, "Пользователи"),
      el("div", { class: "stats" },
        stat(s.approved, "с доступом"), stat(s.pending, "ждут решения"),
        stat(s.rejected, "отклонено"), stat(s.deleted, "удалено"), stat(s.tokens_active, "активных токенов")));
    const telemt = el("div", { class: "card" }, el("h2", {}, "telemt"),
      el("p", { class: "muted small" }, `Режим: ${MODE_NAMES[s.backend_mode] || s.backend_mode}` + (s.web_proxy_host ? ` · WEB-прокси: ${s.web_proxy_host}` : "")));
    if (s.telemt) {
      telemt.append(el("div", { class: "stats" },
        stat(fmtUptime(s.telemt.uptime_seconds), "аптайм"),
        stat(s.telemt.configured_users, "пользователей в конфиге"),
        stat(s.telemt.connections_total, "соединений всего", "с момента запуска telemt"),
        stat(s.telemt.connections_bad_total, "отклонённых соединений", "неверный секрет или не MTProto"),
        stat(s.telemt.handshake_timeouts_total, "незавершённых рукопожатий", "в основном сканеры интернета")),
        el("p", { class: "muted small" },
          "Отклонённые соединения и таймауты рукопожатия — фон публичного прокси: его постоянно сканируют. " +
          "О проблемах у пользователей они сами по себе не говорят; следите за уведомлениями бота и Zabbix."));
    } else {
      telemt.append(el("p", { class: "danger" }, s.telemt_error || "Статистика недоступна"));
    }
    render(online, users, telemt);
  }

  function applyTheme() {
    const dark = tg && tg.colorScheme === "dark";
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  }

  // ---------- start ----------

  async function start() {
    if (!tg || !initData) {
      render(el("div", { class: "card" },
        el("h2", {}, "Откройте в Telegram"),
        el("p", { class: "muted" }, "Это приложение работает только внутри Telegram: откройте бота и нажмите кнопку меню.")));
      return;
    }
    tg.ready();
    tg.expand();
    applyTheme();
    if (tg.onEvent) tg.onEvent("themeChanged", applyTheme);
    if (tg.BackButton) tg.BackButton.onClick(() => backHandler && backHandler());
    try {
      me = await api("/api/me");
    } catch (e) {
      render(errorCard(e));
      return;
    }
    openTab("access");
  }

  start();
})();

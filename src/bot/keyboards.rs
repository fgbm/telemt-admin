//! Inline-клавиатуры для экранов и wizard-сценариев.

use crate::bot::handlers::callback_data::{CallbackAction, ServiceAction, UserLimitField};
use crate::runtime::RuntimeCapabilities;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

fn page_nav_row(
    page: i64,
    total_pages: i64,
    previous: CallbackAction,
    current: CallbackAction,
    next: CallbackAction,
) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("⬅️", previous.encode()),
        InlineKeyboardButton::callback(
            format!("📄 {}/{}", page, total_pages.max(1)),
            current.encode(),
        ),
        InlineKeyboardButton::callback("➡️", next.encode()),
    ]
}

fn refresh_home_row(refresh: CallbackAction) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("🔄 Обновить", refresh.encode()),
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]
}

fn refresh_lookup_row(
    refresh: CallbackAction,
    lookup: CallbackAction,
) -> Vec<InlineKeyboardButton> {
    vec![
        InlineKeyboardButton::callback("🔄 Обновить", refresh.encode()),
        InlineKeyboardButton::callback("🔎 Найти", lookup.encode()),
    ]
}

pub fn approve_reject_buttons(request_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![
        InlineKeyboardButton::callback(
            "✅ Одобрить",
            CallbackAction::ApproveRequest {
                request_id,
                page: 1,
            }
            .encode(),
        ),
        InlineKeyboardButton::callback(
            "❌ Отклонить",
            CallbackAction::RejectRequest {
                request_id,
                page: 1,
            }
            .encode(),
        ),
    ])
}

pub fn user_home_keyboard(has_access: bool) -> InlineKeyboardMarkup {
    if has_access {
        InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback(
                    "🔗 Получить ссылку",
                    CallbackAction::ShowUserLink.encode(),
                ),
                InlineKeyboardButton::callback(
                    "❓ Инструкция",
                    CallbackAction::ShowUsageGuide.encode(),
                ),
            ],
            vec![InlineKeyboardButton::callback(
                "🔄 Обновить статус",
                CallbackAction::ShowUserHome.encode(),
            )],
        ])
    } else {
        InlineKeyboardMarkup::new(vec![
            vec![InlineKeyboardButton::callback(
                "🔑 Ввести invite-токен",
                CallbackAction::PromptInviteToken.encode(),
            )],
            vec![InlineKeyboardButton::callback(
                "🔗 Получить ссылку",
                CallbackAction::ShowUserLink.encode(),
            )],
            vec![InlineKeyboardButton::callback(
                "🔄 Обновить статус",
                CallbackAction::ShowUserHome.encode(),
            )],
        ])
    }
}

pub fn guide_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default().append_row(vec![InlineKeyboardButton::callback(
        "⬅️ Назад",
        CallbackAction::ShowUserHome.encode(),
    )])
}

pub fn admin_home_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "📥 Заявки",
                CallbackAction::ShowPendingRequests.encode(),
            ),
            InlineKeyboardButton::callback(
                "👥 Пользователи",
                CallbackAction::ShowUsersPage { page: 1 }.encode(),
            ),
        ],
        vec![
            InlineKeyboardButton::callback("🎟 Токены", CallbackAction::ShowTokenMenu.encode()),
            InlineKeyboardButton::callback("⚙️ Сервис", CallbackAction::ShowServicePanel.encode()),
        ],
        vec![
            InlineKeyboardButton::callback(
                "📊 Статистика",
                CallbackAction::ShowStats.encode(),
            ),
            InlineKeyboardButton::callback(
                "📢 Рассылка",
                CallbackAction::PromptBroadcastApproved.encode(),
            ),
        ],
        vec![
            InlineKeyboardButton::callback(
                "📁 Группы",
                CallbackAction::ShowGroupsMenu.encode(),
            ),
            InlineKeyboardButton::callback(
                "📥 Импорт из telemt",
                CallbackAction::PromptImportUser.encode(),
            ),
        ],
    ])
}

pub fn groups_menu_keyboard(groups: &[crate::db::UserGroup]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for g in groups {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("📁 {}", g.name),
            CallbackAction::OpenGroupCard { group_id: g.id }.encode(),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "➕ Новая группа",
        CallbackAction::PromptCreateGroup.encode(),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        "⬅️ Админка",
        CallbackAction::ShowAdminHome.encode(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn group_card_keyboard(group_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "🗓 Задать/изменить срок",
                CallbackAction::PromptGroupExpiry { group_id }.encode(),
            ),
            InlineKeyboardButton::callback(
                "♻️ Снять срок",
                CallbackAction::ClearGroupExpiry { group_id }.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "⛔ Отключить всех",
            CallbackAction::GroupDeactivateAll { group_id }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "📅 Применить срок группы ко всем",
            CallbackAction::GroupApplyExpiry { group_id }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ К группам",
            CallbackAction::ShowGroupsMenu.encode(),
        )],
    ])
}

pub fn user_group_picker_keyboard(
    tg_user_id: i64,
    page: i64,
    groups: &[crate::db::UserGroup],
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    rows.push(vec![InlineKeyboardButton::callback(
        "⛔ Не в группе",
        CallbackAction::AssignUserToGroup {
            tg_user_id,
            group_id: 0,
            page,
        }
        .encode(),
    )]);
    for g in groups {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("📁 {}", g.name),
            CallbackAction::AssignUserToGroup {
                tg_user_id,
                group_id: g.id,
                page,
            }
            .encode(),
        )]);
    }
    rows.push(vec![InlineKeyboardButton::callback(
        "⬅️ К карточке",
        CallbackAction::OpenUserCard { tg_user_id, page }.encode(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn pending_requests_keyboard(
    requests: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = requests
        .iter()
        .map(|(request_id, title)| {
            vec![InlineKeyboardButton::callback(
                title.clone(),
                CallbackAction::OpenPendingRequest {
                    request_id: *request_id,
                    page,
                }
                .encode(),
            )]
        })
        .collect();

    let total_pages = total_pages.max(1);
    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        if page > 1 {
            CallbackAction::ShowPendingRequestsPage { page: prev_page }
        } else {
            CallbackAction::Noop
        },
        CallbackAction::Noop,
        if page < total_pages {
            CallbackAction::ShowPendingRequestsPage { page: next_page }
        } else {
            CallbackAction::Noop
        },
    ));
    rows.push(refresh_home_row(CallbackAction::ShowPendingRequestsPage {
        page,
    }));

    InlineKeyboardMarkup::new(rows)
}

pub fn pending_request_card_keyboard(request_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "✅ Одобрить",
                CallbackAction::ApproveRequest { request_id, page }.encode(),
            ),
            InlineKeyboardButton::callback(
                "❌ Отклонить",
                CallbackAction::RejectRequest { request_id, page }.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "⬅️ Назад к заявкам",
            CallbackAction::ShowPendingRequestsPage { page }.encode(),
        )],
    ])
}

pub fn pending_result_keyboard(page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "⬅️ К заявкам",
            CallbackAction::ShowPendingRequestsPage { page }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "🏠 Главная",
            CallbackAction::ShowAdminHome.encode(),
        )],
    ])
}

fn truncate_callback_button_label(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let take = max_chars.saturating_sub(1);
    format!(
        "{}…",
        text.chars().take(take).collect::<String>()
    )
}

/// Кнопки выбора пользователя после частичного поиска (одна кнопка — одна строка).
pub fn user_lookup_candidates_keyboard(
    candidates: &[(i64, String)],
    page: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = candidates
        .iter()
        .map(|(tg_user_id, label)| {
            vec![InlineKeyboardButton::callback(
                truncate_callback_button_label(label, 54),
                CallbackAction::OpenUserCard {
                    tg_user_id: *tg_user_id,
                    page,
                }
                .encode(),
            )]
        })
        .collect();
    rows.push(vec![InlineKeyboardButton::callback(
        "⬅️ К списку пользователей",
        CallbackAction::ShowUsersPage { page }.encode(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn users_page_keyboard(
    users: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (tg_user_id, title) in users {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("👤 {}", title),
            CallbackAction::OpenUserCard {
                tg_user_id: *tg_user_id,
                page,
            }
            .encode(),
        )]);
    }

    let total_pages = total_pages.max(1);
    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        if page > 1 {
            CallbackAction::ShowUsersPage { page: prev_page }
        } else {
            CallbackAction::Noop
        },
        CallbackAction::Noop,
        if page < total_pages {
            CallbackAction::ShowUsersPage { page: next_page }
        } else {
            CallbackAction::Noop
        },
    ));
    rows.push(refresh_lookup_row(
        CallbackAction::ShowUsersPage { page },
        CallbackAction::PromptUserLookup { page },
    ));
    rows.push(vec![InlineKeyboardButton::callback(
        "⛔ Удалить",
        CallbackAction::PromptDeleteUser.encode(),
    )]);
    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Главная",
        CallbackAction::ShowAdminHome.encode(),
    )]);

    InlineKeyboardMarkup::new(rows)
}

pub fn user_card_keyboard(tg_user_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::default()
        .append_row(vec![
            InlineKeyboardButton::callback(
                "🔄 Обновить",
                CallbackAction::OpenUserCard { tg_user_id, page }.encode(),
            ),
            InlineKeyboardButton::callback(
                "🪄 Deep link",
                CallbackAction::SendUserStartLink { tg_user_id }.encode(),
            ),
        ])
        .append_row(vec![InlineKeyboardButton::callback(
            "📁 Группа",
            CallbackAction::UserGroupPicker { tg_user_id, page }.encode(),
        )])
        .append_row(vec![InlineKeyboardButton::callback(
            "🔗 Данные + QR",
            CallbackAction::ViewUserQr { tg_user_id }.encode(),
        )])
        .append_row(vec![
            InlineKeyboardButton::callback(
                "TCP лимит",
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field: UserLimitField::MaxTcpConns,
                }
                .encode(),
            ),
            InlineKeyboardButton::callback(
                "IP лимит",
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field: UserLimitField::MaxUniqueIps,
                }
                .encode(),
            ),
        ])
        .append_row(vec![
            InlineKeyboardButton::callback(
                "Квота",
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field: UserLimitField::DataQuotaBytes,
                }
                .encode(),
            ),
            InlineKeyboardButton::callback(
                "Истекает",
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field: UserLimitField::Expiration,
                }
                .encode(),
            ),
        ])
        .append_row(vec![InlineKeyboardButton::callback(
            "⛔ Удалить пользователя",
            CallbackAction::ConfirmUserBan { tg_user_id, page }.encode(),
        )])
        .append_row(vec![InlineKeyboardButton::callback(
            "⬅️ Назад к списку",
            CallbackAction::ShowUsersPage { page }.encode(),
        )])
}

pub fn service_control_buttons(caps: &RuntimeCapabilities) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    let mut row_refresh = vec![InlineKeyboardButton::callback(
        "🔄 Обновить",
        CallbackAction::ShowServicePanel.encode(),
    )];
    if caps.can_reload_config {
        row_refresh.push(InlineKeyboardButton::callback(
            "📖 Reload",
            CallbackAction::ExecuteServiceAction {
                action: ServiceAction::Reload,
            }
            .encode(),
        ));
    }
    rows.push(row_refresh);

    rows.push(vec![InlineKeyboardButton::callback(
        "📈 Top пользователей",
        CallbackAction::ShowConnectionsSummary.encode(),
    )]);

    if caps.can_restart || caps.can_stop {
        let mut row_risky = Vec::new();
        if caps.can_restart {
            row_risky.push(InlineKeyboardButton::callback(
                "🔄 Перезапустить",
                CallbackAction::ConfirmServiceAction {
                    action: ServiceAction::Restart,
                }
                .encode(),
            ));
        }
        if caps.can_stop {
            row_risky.push(InlineKeyboardButton::callback(
                "⏹ Остановить",
                CallbackAction::ConfirmServiceAction {
                    action: ServiceAction::Stop,
                }
                .encode(),
            ));
        }
        if !row_risky.is_empty() {
            rows.push(row_risky);
        }
    }

    if caps.can_start {
        rows.push(vec![
            InlineKeyboardButton::callback(
                "▶️ Запустить",
                CallbackAction::ExecuteServiceAction {
                    action: ServiceAction::Start,
                }
                .encode(),
            ),
            InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
        ]);
    } else {
        rows.push(vec![InlineKeyboardButton::callback(
            "🏠 Главная",
            CallbackAction::ShowAdminHome.encode(),
        )]);
    }

    InlineKeyboardMarkup::new(rows)
}

pub fn token_menu_keyboard(auto_approve_enabled: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![InlineKeyboardButton::callback(
            "📋 Список токенов",
            CallbackAction::ShowTokenListPage { page: 1 }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "🎫 Создать ручной токен",
            CallbackAction::PromptTokenCreate {
                auto_approve: false,
            }
            .encode(),
        )],
    ];

    if auto_approve_enabled {
        rows.insert(
            1,
            vec![InlineKeyboardButton::callback(
                "🚀 Создать авто-токен",
                CallbackAction::PromptTokenCreate { auto_approve: true }.encode(),
            )],
        );
    }

    rows.push(vec![InlineKeyboardButton::callback(
        "🏠 Главная",
        CallbackAction::ShowAdminHome.encode(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn token_list_keyboard(
    tokens: &[(i64, String)],
    page: i64,
    total_pages: i64,
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for (token_id, title) in tokens {
        rows.push(vec![InlineKeyboardButton::callback(
            format!("🎟 {}", title),
            CallbackAction::OpenTokenCard {
                token_id: *token_id,
                page,
            }
            .encode(),
        )]);
    }

    let total_pages = total_pages.max(1);
    let prev_page = if page > 1 { page - 1 } else { 1 };
    let next_page = if page < total_pages {
        page + 1
    } else {
        total_pages
    };

    rows.push(page_nav_row(
        page,
        total_pages,
        if page > 1 {
            CallbackAction::ShowTokenListPage { page: prev_page }
        } else {
            CallbackAction::Noop
        },
        CallbackAction::Noop,
        if page < total_pages {
            CallbackAction::ShowTokenListPage { page: next_page }
        } else {
            CallbackAction::Noop
        },
    ));
    rows.push(refresh_lookup_row(
        CallbackAction::ShowTokenListPage { page },
        CallbackAction::PromptTokenLookup { page },
    ));
    rows.push(vec![
        InlineKeyboardButton::callback("⬅️ Назад", CallbackAction::ShowTokenMenu.encode()),
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]);

    InlineKeyboardMarkup::new(rows)
}

pub fn token_card_keyboard(token_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "🔄 Обновить",
                CallbackAction::OpenTokenCard { token_id, page }.encode(),
            ),
            InlineKeyboardButton::callback(
                "🔗 Ссылка",
                CallbackAction::SendTokenStartLink { token_id }.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "🗑 Отозвать токен",
            CallbackAction::ConfirmTokenRevoke { token_id, page }.encode(),
        )],
        vec![InlineKeyboardButton::callback(
            "⬅️ Назад к списку",
            CallbackAction::ShowTokenListPage { page }.encode(),
        )],
    ])
}

pub fn confirm_token_revoke_keyboard(token_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteTokenRevoke { token_id, page }.encode(),
        ),
        InlineKeyboardButton::callback(
            "⬅️ Назад",
            CallbackAction::OpenTokenCard { token_id, page }.encode(),
        ),
    ]])
}

pub fn confirm_service_action_keyboard(action: ServiceAction) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteServiceAction { action }.encode(),
        ),
        InlineKeyboardButton::callback("⬅️ Назад", CallbackAction::ShowServicePanel.encode()),
    ]])
}

pub fn stats_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("🔄 Обновить", CallbackAction::ShowStats.encode()),
        InlineKeyboardButton::callback(
            "📈 Top users",
            CallbackAction::ShowConnectionsSummary.encode(),
        ),
    ], vec![
        InlineKeyboardButton::callback("🏠 Главная", CallbackAction::ShowAdminHome.encode()),
    ]])
}

pub fn connections_summary_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback(
                "🔄 Обновить",
                CallbackAction::ShowConnectionsSummary.encode(),
            ),
            InlineKeyboardButton::callback(
                "⚙️ Сервис",
                CallbackAction::ShowServicePanel.encode(),
            ),
        ],
        vec![InlineKeyboardButton::callback(
            "🏠 Главная",
            CallbackAction::ShowAdminHome.encode(),
        )],
    ])
}

pub fn cancel_keyboard(back_action: CallbackAction) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("⬅️ Назад", back_action.encode()),
        InlineKeyboardButton::callback("✖️ Отмена", CallbackAction::CancelWizard.encode()),
    ]])
}

pub fn confirm_delete_keyboard(tg_user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Да, удалить",
            CallbackAction::ExecuteDeleteUser { tg_user_id }.encode(),
        ),
        InlineKeyboardButton::callback("✖️ Отмена", CallbackAction::ShowAdminHome.encode()),
    ]])
}

pub fn confirm_user_ban_keyboard(tg_user_id: i64, page: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Подтвердить",
            CallbackAction::ExecuteUserBan { tg_user_id, page }.encode(),
        ),
        InlineKeyboardButton::callback(
            "⬅️ Назад",
            CallbackAction::OpenUserCard { tg_user_id, page }.encode(),
        ),
    ]])
}

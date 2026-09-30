#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Reload,
    Status,
}

impl ServiceAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Reload => "reload",
            Self::Status => "status",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "restart" => Some(Self::Restart),
            "reload" => Some(Self::Reload),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLimitField {
    MaxTcpConns,
    DataQuotaBytes,
    MaxUniqueIps,
    Expiration,
}

impl UserLimitField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxTcpConns => "tcp",
            Self::DataQuotaBytes => "quota",
            Self::MaxUniqueIps => "ips",
            Self::Expiration => "expire",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tcp" => Some(Self::MaxTcpConns),
            "quota" => Some(Self::DataQuotaBytes),
            "ips" => Some(Self::MaxUniqueIps),
            "expire" => Some(Self::Expiration),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallbackAction {
    Noop,
    ShowAdminHome,
    ShowUserHome,
    ShowUserLink,
    ShowUsageGuide,
    PromptInviteToken,
    CancelWizard,
    ShowPendingRequests,
    ShowPendingRequestsPage {
        page: i64,
    },
    OpenPendingRequest {
        request_id: i64,
        page: i64,
    },
    ShowUsersPage {
        page: i64,
    },
    PromptUserLookup {
        page: i64,
    },
    OpenUserCard {
        tg_user_id: i64,
        page: i64,
    },
    PromptUserLimit {
        tg_user_id: i64,
        page: i64,
        field: UserLimitField,
    },
    ViewUserQr {
        tg_user_id: i64,
    },
    SendUserStartLink {
        tg_user_id: i64,
    },
    ConfirmUserBan {
        tg_user_id: i64,
        page: i64,
    },
    ExecuteUserBan {
        tg_user_id: i64,
        page: i64,
    },
    ShowStats,
    ShowServicePanel,
    ShowConnectionsSummary,
    ConfirmServiceAction {
        action: ServiceAction,
    },
    ExecuteServiceAction {
        action: ServiceAction,
    },
    ShowTokenMenu,
    PromptTokenCreate {
        auto_approve: bool,
    },
    ShowTokenList,
    ShowTokenListPage {
        page: i64,
    },
    PromptTokenLookup {
        page: i64,
    },
    OpenTokenCard {
        token_id: i64,
        page: i64,
    },
    SendTokenStartLink {
        token_id: i64,
    },
    ConfirmTokenRevoke {
        token_id: i64,
        page: i64,
    },
    ExecuteTokenRevoke {
        token_id: i64,
        page: i64,
    },
    PromptDeleteUser,
    ExecuteDeleteUser {
        tg_user_id: i64,
    },
    ApproveRequest {
        request_id: i64,
        page: i64,
    },
    RejectRequest {
        request_id: i64,
        page: i64,
    },
    /// Рассылка сообщения всем пользователям со статусом approved.
    PromptBroadcastApproved,
    ShowGroupsMenu,
    OpenGroupCard {
        group_id: i64,
    },
    PromptCreateGroup,
    PromptGroupExpiry {
        group_id: i64,
    },
    ClearGroupExpiry {
        group_id: i64,
    },
    GroupDeactivateAll {
        group_id: i64,
    },
    GroupApplyExpiry {
        group_id: i64,
    },
    UserGroupPicker {
        tg_user_id: i64,
        page: i64,
    },
    AssignUserToGroup {
        tg_user_id: i64,
        group_id: i64,
        page: i64,
    },
    PromptImportUser,
}

impl CallbackAction {
    pub fn encode(&self) -> String {
        match self {
            Self::Noop => "v1|noop".to_string(),
            Self::ShowAdminHome => "v1|admin|home".to_string(),
            Self::ShowUserHome => "v1|user|home".to_string(),
            Self::ShowUserLink => "v1|user|link".to_string(),
            Self::ShowUsageGuide => "v1|user|guide".to_string(),
            Self::PromptInviteToken => "v1|user|invite".to_string(),
            Self::CancelWizard => "v1|wizard|cancel".to_string(),
            Self::ShowPendingRequests => "v1|admin|pending".to_string(),
            Self::ShowPendingRequestsPage { page } => format!("v1|admin|pending|page|{page}"),
            Self::OpenPendingRequest { request_id, page } => {
                format!("v1|admin|pending|open|{request_id}|{page}")
            }
            Self::ShowUsersPage { page } => format!("v1|admin|users|page|{page}"),
            Self::PromptUserLookup { page } => format!("v1|admin|users|lookup|{page}"),
            Self::OpenUserCard { tg_user_id, page } => {
                format!("v1|admin|user|open|{tg_user_id}|{page}")
            }
            Self::PromptUserLimit {
                tg_user_id,
                page,
                field,
            } => format!("v1|admin|user|limit|{}|{tg_user_id}|{page}", field.as_str()),
            Self::ViewUserQr { tg_user_id } => format!("v1|admin|user|view|{tg_user_id}"),
            Self::SendUserStartLink { tg_user_id } => {
                format!("v1|admin|user|startlink|{tg_user_id}")
            }
            Self::ConfirmUserBan { tg_user_id, page } => {
                format!("v1|admin|user|ban_confirm|{tg_user_id}|{page}")
            }
            Self::ExecuteUserBan { tg_user_id, page } => {
                format!("v1|admin|user|ban_execute|{tg_user_id}|{page}")
            }
            Self::ShowStats => "v1|admin|stats".to_string(),
            Self::ShowServicePanel => "v1|admin|service".to_string(),
            Self::ShowConnectionsSummary => "v1|admin|service|connections".to_string(),
            Self::ConfirmServiceAction { action } => {
                format!("v1|admin|service|confirm|{}", action.as_str())
            }
            Self::ExecuteServiceAction { action } => {
                format!("v1|admin|service|execute|{}", action.as_str())
            }
            Self::ShowTokenMenu => "v1|admin|token".to_string(),
            Self::PromptTokenCreate { auto_approve } => {
                let auto_approve = if *auto_approve { 1 } else { 0 };
                format!("v1|admin|token|create|{auto_approve}")
            }
            Self::ShowTokenList => "v1|admin|token|list".to_string(),
            Self::ShowTokenListPage { page } => format!("v1|admin|token|page|{page}"),
            Self::PromptTokenLookup { page } => format!("v1|admin|token|lookup|{page}"),
            Self::OpenTokenCard { token_id, page } => {
                format!("v1|admin|token|open|{token_id}|{page}")
            }
            Self::SendTokenStartLink { token_id } => {
                format!("v1|admin|token|startlink|{token_id}")
            }
            Self::ConfirmTokenRevoke { token_id, page } => {
                format!("v1|admin|token|revoke|confirm|{token_id}|{page}")
            }
            Self::ExecuteTokenRevoke { token_id, page } => {
                format!("v1|admin|token|revoke|execute|{token_id}|{page}")
            }
            Self::PromptDeleteUser => "v1|admin|delete".to_string(),
            Self::ExecuteDeleteUser { tg_user_id } => {
                format!("v1|admin|delete|execute|{tg_user_id}")
            }
            Self::ApproveRequest { request_id, page } => {
                format!("v1|req|approve|{request_id}|{page}")
            }
            Self::RejectRequest { request_id, page } => {
                format!("v1|req|reject|{request_id}|{page}")
            }
            Self::PromptBroadcastApproved => "v1|admin|broadcast".to_string(),
            Self::ShowGroupsMenu => "v1|admin|groups".to_string(),
            Self::OpenGroupCard { group_id } => format!("v1|admin|groups|open|{group_id}"),
            Self::PromptCreateGroup => "v1|admin|groups|create".to_string(),
            Self::PromptGroupExpiry { group_id } => {
                format!("v1|admin|groups|expiry|prompt|{group_id}")
            }
            Self::ClearGroupExpiry { group_id } => {
                format!("v1|admin|groups|expiry|clear|{group_id}")
            }
            Self::GroupDeactivateAll { group_id } => {
                format!("v1|admin|groups|deactivate|{group_id}")
            }
            Self::GroupApplyExpiry { group_id } => {
                format!("v1|admin|groups|apply_expiry|{group_id}")
            }
            Self::UserGroupPicker { tg_user_id, page } => {
                format!("v1|admin|user|group|pick|{tg_user_id}|{page}")
            }
            Self::AssignUserToGroup {
                tg_user_id,
                group_id,
                page,
            } => format!("v1|admin|user|group|set|{tg_user_id}|{group_id}|{page}"),
            Self::PromptImportUser => "v1|admin|import".to_string(),
        }
    }

    pub fn decode(data: &str) -> Option<Self> {
        Self::decode_v1(data)
    }

    fn decode_v1(data: &str) -> Option<Self> {
        let parts: Vec<&str> = data.split('|').collect();
        match parts.as_slice() {
            ["v1", "noop"] => Some(Self::Noop),
            ["v1", "admin", "home"] => Some(Self::ShowAdminHome),
            ["v1", "user", "home"] => Some(Self::ShowUserHome),
            ["v1", "user", "link"] => Some(Self::ShowUserLink),
            ["v1", "user", "guide"] => Some(Self::ShowUsageGuide),
            ["v1", "user", "invite"] => Some(Self::PromptInviteToken),
            ["v1", "wizard", "cancel"] => Some(Self::CancelWizard),
            ["v1", "admin", "pending"] => Some(Self::ShowPendingRequests),
            ["v1", "admin", "pending", "page", page] => Some(Self::ShowPendingRequestsPage {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "pending", "open", request_id, page] => {
                Some(Self::OpenPendingRequest {
                    request_id: parse_i64(request_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "users", "page", page] => Some(Self::ShowUsersPage {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "users", "lookup", page] => Some(Self::PromptUserLookup {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "user", "open", tg_user_id, page] => Some(Self::OpenUserCard {
                tg_user_id: parse_i64(tg_user_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "user", "limit", field, tg_user_id, page] => {
                Some(Self::PromptUserLimit {
                    field: UserLimitField::parse(field)?,
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "user", "view", tg_user_id] => Some(Self::ViewUserQr {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "admin", "user", "startlink", tg_user_id] => Some(Self::SendUserStartLink {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "admin", "user", "ban_confirm", tg_user_id, page] => {
                Some(Self::ConfirmUserBan {
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "user", "ban_execute", tg_user_id, page] => {
                Some(Self::ExecuteUserBan {
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "stats"] => Some(Self::ShowStats),
            ["v1", "admin", "service"] => Some(Self::ShowServicePanel),
            ["v1", "admin", "service", "connections"] => Some(Self::ShowConnectionsSummary),
            ["v1", "admin", "service", "confirm", action] => Some(Self::ConfirmServiceAction {
                action: ServiceAction::parse(action)?,
            }),
            ["v1", "admin", "service", "execute", action] => Some(Self::ExecuteServiceAction {
                action: ServiceAction::parse(action)?,
            }),
            ["v1", "admin", "token"] => Some(Self::ShowTokenMenu),
            ["v1", "admin", "token", "create", auto_approve] => Some(Self::PromptTokenCreate {
                auto_approve: *auto_approve == "1",
            }),
            ["v1", "admin", "token", "list"] => Some(Self::ShowTokenList),
            ["v1", "admin", "token", "page", page] => Some(Self::ShowTokenListPage {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "token", "lookup", page] => Some(Self::PromptTokenLookup {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "token", "open", token_id, page] => Some(Self::OpenTokenCard {
                token_id: parse_i64(token_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "token", "startlink", token_id] => Some(Self::SendTokenStartLink {
                token_id: parse_i64(token_id)?,
            }),
            ["v1", "admin", "token", "revoke", "confirm", token_id, page] => {
                Some(Self::ConfirmTokenRevoke {
                    token_id: parse_i64(token_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "token", "revoke", "execute", token_id, page] => {
                Some(Self::ExecuteTokenRevoke {
                    token_id: parse_i64(token_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "delete"] => Some(Self::PromptDeleteUser),
            ["v1", "admin", "delete", "execute", tg_user_id] => Some(Self::ExecuteDeleteUser {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "req", "approve", request_id, page] => Some(Self::ApproveRequest {
                request_id: parse_i64(request_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "req", "reject", request_id, page] => Some(Self::RejectRequest {
                request_id: parse_i64(request_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "broadcast"] => Some(Self::PromptBroadcastApproved),
            ["v1", "admin", "groups"] => Some(Self::ShowGroupsMenu),
            ["v1", "admin", "groups", "open", group_id] => Some(Self::OpenGroupCard {
                group_id: parse_i64(group_id)?,
            }),
            ["v1", "admin", "groups", "create"] => Some(Self::PromptCreateGroup),
            ["v1", "admin", "groups", "expiry", "prompt", group_id] => {
                Some(Self::PromptGroupExpiry {
                    group_id: parse_i64(group_id)?,
                })
            }
            ["v1", "admin", "groups", "expiry", "clear", group_id] => {
                Some(Self::ClearGroupExpiry {
                    group_id: parse_i64(group_id)?,
                })
            }
            ["v1", "admin", "groups", "deactivate", group_id] => Some(Self::GroupDeactivateAll {
                group_id: parse_i64(group_id)?,
            }),
            ["v1", "admin", "groups", "apply_expiry", group_id] => Some(Self::GroupApplyExpiry {
                group_id: parse_i64(group_id)?,
            }),
            ["v1", "admin", "user", "group", "pick", tg_user_id, page] => {
                Some(Self::UserGroupPicker {
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            [
                "v1",
                "admin",
                "user",
                "group",
                "set",
                tg_user_id,
                group_id,
                page,
            ] => Some(Self::AssignUserToGroup {
                tg_user_id: parse_i64(tg_user_id)?,
                group_id: parse_i64(group_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "import"] => Some(Self::PromptImportUser),
            _ => None,
        }
    }
}

fn parse_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::{CallbackAction, ServiceAction, UserLimitField};

    #[test]
    fn service_action_parse_accepts_known_values() {
        assert_eq!(ServiceAction::parse("start"), Some(ServiceAction::Start));
        assert_eq!(ServiceAction::parse("stop"), Some(ServiceAction::Stop));
        assert_eq!(
            ServiceAction::parse("restart"),
            Some(ServiceAction::Restart)
        );
        assert_eq!(ServiceAction::parse("reload"), Some(ServiceAction::Reload));
        assert_eq!(ServiceAction::parse("status"), Some(ServiceAction::Status));
        assert_eq!(ServiceAction::parse("unknown"), None);
    }

    #[test]
    fn user_limit_field_parse_accepts_known_values() {
        assert_eq!(
            UserLimitField::parse("tcp"),
            Some(UserLimitField::MaxTcpConns)
        );
        assert_eq!(
            UserLimitField::parse("quota"),
            Some(UserLimitField::DataQuotaBytes)
        );
        assert_eq!(
            UserLimitField::parse("ips"),
            Some(UserLimitField::MaxUniqueIps)
        );
        assert_eq!(
            UserLimitField::parse("expire"),
            Some(UserLimitField::Expiration)
        );
        assert_eq!(UserLimitField::parse("bad"), None);
    }

    #[test]
    fn callback_action_roundtrip_preserves_payload() {
        let cases = [
            CallbackAction::Noop,
            CallbackAction::ShowPendingRequestsPage { page: 3 },
            CallbackAction::OpenPendingRequest {
                request_id: 42,
                page: 2,
            },
            CallbackAction::PromptUserLimit {
                tg_user_id: 1001,
                page: 4,
                field: UserLimitField::DataQuotaBytes,
            },
            CallbackAction::ConfirmServiceAction {
                action: ServiceAction::Reload,
            },
            CallbackAction::PromptTokenCreate { auto_approve: true },
            CallbackAction::OpenTokenCard {
                token_id: 55,
                page: 7,
            },
            CallbackAction::AssignUserToGroup {
                tg_user_id: 12,
                group_id: 99,
                page: 5,
            },
            CallbackAction::ShowGroupsMenu,
            CallbackAction::OpenGroupCard { group_id: 7 },
            CallbackAction::PromptCreateGroup,
            CallbackAction::PromptGroupExpiry { group_id: 8 },
            CallbackAction::ClearGroupExpiry { group_id: 9 },
            CallbackAction::GroupDeactivateAll { group_id: 3 },
            CallbackAction::GroupApplyExpiry { group_id: 4 },
            CallbackAction::UserGroupPicker {
                tg_user_id: 100,
                page: 2,
            },
            CallbackAction::AssignUserToGroup {
                tg_user_id: 12,
                group_id: 0,
                page: 1,
            },
            CallbackAction::PromptImportUser,
            CallbackAction::PromptBroadcastApproved,
        ];

        for case in cases {
            let encoded = case.encode();
            assert_eq!(CallbackAction::decode(&encoded), Some(case));
        }
    }

    #[test]
    fn decode_clamps_page_to_one() {
        assert_eq!(
            CallbackAction::decode("v1|admin|pending|page|0"),
            Some(CallbackAction::ShowPendingRequestsPage { page: 1 })
        );
        assert_eq!(
            CallbackAction::decode("v1|admin|user|open|12|-9"),
            Some(CallbackAction::OpenUserCard {
                tg_user_id: 12,
                page: 1,
            })
        );
    }

    #[test]
    fn decode_rejects_invalid_payloads() {
        assert_eq!(CallbackAction::decode("v1|admin|service|confirm|bad"), None);
        assert_eq!(CallbackAction::decode("v1|admin|user|limit|bad|1|2"), None);
        assert_eq!(CallbackAction::decode("v1|admin|pending|open|abc|2"), None);
        assert_eq!(CallbackAction::decode("v2|admin|home"), None);
        assert_eq!(CallbackAction::decode("garbage"), None);
    }
}

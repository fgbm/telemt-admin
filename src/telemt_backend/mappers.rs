use super::api_dto::{ApiUserInfo, RuntimeConnectionUserData, UserLinks};
use super::types::{
    TelemtBackendMode, TelemtConnectionTopUser, TelemtConnectionsSummary, TelemtUserInfo,
};

pub(super) fn pick_best_link(links: &UserLinks) -> Option<String> {
    links
        .tls
        .first()
        .cloned()
        .or_else(|| links.secure.first().cloned())
        .or_else(|| links.classic.first().cloned())
}

fn collect_links(links: &UserLinks) -> Vec<String> {
    links
        .tls
        .iter()
        .chain(links.secure.iter())
        .chain(links.classic.iter())
        .cloned()
        .collect()
}

pub(super) fn map_api_user_info(source: TelemtBackendMode, user: ApiUserInfo) -> TelemtUserInfo {
    TelemtUserInfo {
        source,
        user_ad_tag: user.user_ad_tag,
        max_tcp_conns: user.max_tcp_conns,
        expiration_rfc3339: user.expiration_rfc3339,
        data_quota_bytes: user.data_quota_bytes,
        max_unique_ips: user.max_unique_ips,
        current_connections: Some(user.current_connections),
        active_unique_ips: Some(user.active_unique_ips),
        active_unique_ips_list: user.active_unique_ips_list,
        recent_unique_ips: Some(user.recent_unique_ips),
        recent_unique_ips_list: user.recent_unique_ips_list,
        total_octets: Some(user.total_octets),
        links: collect_links(&user.links),
    }
}

pub(super) fn map_connection_top_user(user: RuntimeConnectionUserData) -> TelemtConnectionTopUser {
    TelemtConnectionTopUser {
        username: user.username,
        current_connections: user.current_connections,
        total_octets: user.total_octets,
    }
}

pub(crate) fn build_summary_from_user_list(
    users: Vec<ApiUserInfo>,
    limit: usize,
) -> TelemtConnectionsSummary {
    let limit = limit.max(1);

    let mut by_connections: Vec<_> = users
        .iter()
        .map(|u| TelemtConnectionTopUser {
            username: u.username.clone(),
            current_connections: u.current_connections,
            total_octets: u.total_octets,
        })
        .collect();
    by_connections.sort_by(|a, b| b.current_connections.cmp(&a.current_connections));
    let top_by_connections = by_connections.into_iter().take(limit).collect();

    let mut by_throughput: Vec<_> = users
        .iter()
        .map(|u| TelemtConnectionTopUser {
            username: u.username.clone(),
            current_connections: u.current_connections,
            total_octets: u.total_octets,
        })
        .collect();
    by_throughput.sort_by(|a, b| b.total_octets.cmp(&a.total_octets));
    let top_by_throughput = by_throughput.into_iter().take(limit).collect();

    let total_connections: u64 = users.iter().map(|u| u.current_connections).sum();
    let active_users = users.iter().filter(|u| u.current_connections > 0).count();

    TelemtConnectionsSummary {
        current_connections: total_connections,
        current_connections_me: 0,
        current_connections_direct: 0,
        active_users,
        top_by_connections,
        top_by_throughput,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_summary_from_user_list, collect_links, map_api_user_info, map_connection_top_user,
        pick_best_link,
    };
    use crate::telemt_backend::api_dto::{ApiUserInfo, RuntimeConnectionUserData, UserLinks};
    use crate::telemt_backend::types::TelemtBackendMode;

    #[test]
    fn pick_best_link_prefers_tls_then_secure_then_classic() {
        let tls_first = UserLinks {
            classic: vec!["classic".to_string()],
            secure: vec!["secure".to_string()],
            tls: vec!["tls".to_string()],
        };
        assert_eq!(pick_best_link(&tls_first).as_deref(), Some("tls"));

        let secure_fallback = UserLinks {
            classic: vec!["classic".to_string()],
            secure: vec!["secure".to_string()],
            tls: Vec::new(),
        };
        assert_eq!(pick_best_link(&secure_fallback).as_deref(), Some("secure"));

        let classic_fallback = UserLinks {
            classic: vec!["classic".to_string()],
            secure: Vec::new(),
            tls: Vec::new(),
        };
        assert_eq!(pick_best_link(&classic_fallback).as_deref(), Some("classic"));
        assert_eq!(
            pick_best_link(&UserLinks {
                classic: Vec::new(),
                secure: Vec::new(),
                tls: Vec::new(),
            }),
            None
        );
    }

    #[test]
    fn collect_links_preserves_all_link_groups() {
        let links = UserLinks {
            classic: vec!["classic-1".to_string()],
            secure: vec!["secure-1".to_string()],
            tls: vec!["tls-1".to_string(), "tls-2".to_string()],
        };

        assert_eq!(
            collect_links(&links),
            vec![
                "tls-1".to_string(),
                "tls-2".to_string(),
                "secure-1".to_string(),
                "classic-1".to_string(),
            ]
        );
    }

    #[test]
    fn map_api_user_info_maps_runtime_fields() {
        let user = ApiUserInfo {
            username: "testuser".to_string(),
            user_ad_tag: Some("promo".to_string()),
            max_tcp_conns: Some(10),
            expiration_rfc3339: Some("2026-04-01T00:00:00Z".to_string()),
            data_quota_bytes: Some(2048),
            max_unique_ips: Some(3),
            current_connections: 2,
            active_unique_ips: 1,
            active_unique_ips_list: vec!["1.1.1.1".to_string()],
            recent_unique_ips: 2,
            recent_unique_ips_list: vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()],
            total_octets: 4096,
            links: UserLinks {
                classic: vec!["classic".to_string()],
                secure: vec!["secure".to_string()],
                tls: vec!["tls".to_string()],
            },
        };

        let mapped = map_api_user_info(TelemtBackendMode::ControlApi, user);

        assert_eq!(mapped.source, TelemtBackendMode::ControlApi);
        assert_eq!(mapped.user_ad_tag.as_deref(), Some("promo"));
        assert_eq!(mapped.max_tcp_conns, Some(10));
        assert_eq!(mapped.data_quota_bytes, Some(2048));
        assert_eq!(mapped.max_unique_ips, Some(3));
        assert_eq!(mapped.current_connections, Some(2));
        assert_eq!(mapped.active_unique_ips, Some(1));
        assert_eq!(mapped.recent_unique_ips, Some(2));
        assert_eq!(mapped.total_octets, Some(4096));
        assert_eq!(mapped.links.len(), 3);
        assert_eq!(mapped.links[0], "tls");
    }

    #[test]
    fn map_connection_top_user_preserves_counters() {
        let mapped = map_connection_top_user(RuntimeConnectionUserData {
            username: "tg_1".to_string(),
            current_connections: 7,
            total_octets: 99,
        });

        assert_eq!(mapped.username, "tg_1");
        assert_eq!(mapped.current_connections, 7);
        assert_eq!(mapped.total_octets, 99);
    }

    #[test]
    fn build_summary_from_empty_user_list_returns_zeros() {
        let summary = build_summary_from_user_list(Vec::new(), 5);
        assert_eq!(summary.current_connections, 0);
        assert_eq!(summary.active_users, 0);
        assert!(summary.top_by_connections.is_empty());
        assert!(summary.top_by_throughput.is_empty());
    }

    #[test]
    fn build_summary_sorts_and_limits_users() {
        let users = vec![
            ApiUserInfo {
                username: "alice".to_string(),
                user_ad_tag: None,
                max_tcp_conns: None,
                expiration_rfc3339: None,
                data_quota_bytes: None,
                max_unique_ips: None,
                current_connections: 10,
                active_unique_ips: 1,
                active_unique_ips_list: Vec::new(),
                recent_unique_ips: 1,
                recent_unique_ips_list: Vec::new(),
                total_octets: 100,
                links: UserLinks {
                    classic: Vec::new(),
                    secure: Vec::new(),
                    tls: Vec::new(),
                },
            },
            ApiUserInfo {
                username: "bob".to_string(),
                user_ad_tag: None,
                max_tcp_conns: None,
                expiration_rfc3339: None,
                data_quota_bytes: None,
                max_unique_ips: None,
                current_connections: 5,
                active_unique_ips: 1,
                active_unique_ips_list: Vec::new(),
                recent_unique_ips: 1,
                recent_unique_ips_list: Vec::new(),
                total_octets: 500,
                links: UserLinks {
                    classic: Vec::new(),
                    secure: Vec::new(),
                    tls: Vec::new(),
                },
            },
            ApiUserInfo {
                username: "charlie".to_string(),
                user_ad_tag: None,
                max_tcp_conns: None,
                expiration_rfc3339: None,
                data_quota_bytes: None,
                max_unique_ips: None,
                current_connections: 1,
                active_unique_ips: 1,
                active_unique_ips_list: Vec::new(),
                recent_unique_ips: 1,
                recent_unique_ips_list: Vec::new(),
                total_octets: 50,
                links: UserLinks {
                    classic: Vec::new(),
                    secure: Vec::new(),
                    tls: Vec::new(),
                },
            },
        ];

        let summary = build_summary_from_user_list(users, 2);

        assert_eq!(summary.current_connections, 16);
        assert_eq!(summary.active_users, 3);

        // By connections: alice (10), bob (5)
        assert_eq!(summary.top_by_connections.len(), 2);
        assert_eq!(summary.top_by_connections[0].username, "alice");
        assert_eq!(summary.top_by_connections[0].current_connections, 10);
        assert_eq!(summary.top_by_connections[1].username, "bob");
        assert_eq!(summary.top_by_connections[1].current_connections, 5);

        // By throughput: bob (500), alice (100)
        assert_eq!(summary.top_by_throughput.len(), 2);
        assert_eq!(summary.top_by_throughput[0].username, "bob");
        assert_eq!(summary.top_by_throughput[0].total_octets, 500);
        assert_eq!(summary.top_by_throughput[1].username, "alice");
        assert_eq!(summary.top_by_throughput[1].total_octets, 100);
    }
}

//! Генерация fake-TLS и WEB-ссылок на прокси telemt.

use crate::config::WebSecretMode;
use crate::telemt_cfg::TelemtLinkParams;
use rand::RngCore;
use std::fmt::Write;

/// Генерирует 32 hex-символа (16 байт) для секрета пользователя.
pub fn generate_user_secret() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Формирует fake-TLS секрет: ee + user_secret (32 hex) + hex(tls_domain).
pub fn build_fake_tls_secret(user_secret: &str, tls_domain: &str) -> String {
    let domain_hex = hex::encode(tls_domain.as_bytes());
    let mut s = String::with_capacity(2 + 32 + domain_hex.len());
    s.push_str("ee");
    s.push_str(user_secret);
    s.push_str(&domain_hex);
    s
}

/// Формирует tg://proxy ссылку.
pub fn build_proxy_link(
    params: &TelemtLinkParams,
    user_secret: &str,
) -> Result<String, std::fmt::Error> {
    let secret = build_fake_tls_secret(user_secret, &params.tls_domain);
    let mut url = String::new();
    write!(
        url,
        "tg://proxy?server={}&port={}&secret={}",
        params.host, params.port, secret
    )?;
    Ok(url)
}

/// Извлекает 32-символьный секрет пользователя из ссылки `tg://proxy` (форматы `ee`, `dd`, classic).
pub fn extract_user_secret(proxy_link: &str) -> Option<String> {
    let (_, query) = proxy_link.split_once('?')?;
    let value = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("secret="))?
        .to_ascii_lowercase();
    if !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    // classic — 32 hex; `dd` — ровно 34; `ee` — 34 + hex(tls_domain).
    let secret = match value.len() {
        32 => value.as_str(),
        34 if value.starts_with("dd") => &value[2..34],
        len if len > 34 && value.starts_with("ee") => &value[2..34],
        _ => return None,
    };
    Some(secret.to_string())
}

/// Формирует ссылку `tg://webproxy` для WEB-прокси telemt (порт всегда 443).
pub fn build_web_proxy_link(host: &str, user_secret: &str, mode: WebSecretMode) -> String {
    let prefix = match mode {
        WebSecretMode::Dd => "dd",
        WebSecretMode::Plain => "",
    };
    format!("tg://webproxy?server={host}&secret={prefix}{user_secret}")
}

#[cfg(test)]
mod tests {
    use super::{
        build_fake_tls_secret, build_proxy_link, build_web_proxy_link, extract_user_secret,
        generate_user_secret,
    };
    use crate::config::WebSecretMode;
    use crate::telemt_cfg::TelemtLinkParams;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn extract_user_secret_reads_fake_tls_link() {
        let link = "tg://proxy?server=proxy.example.com&port=443&secret=ee0123456789abcdef0123456789abcdef6578616d706c652e636f6d";

        assert_eq!(extract_user_secret(link).as_deref(), Some(SECRET));
    }

    #[test]
    fn extract_user_secret_reads_dd_and_classic_links() {
        let dd = format!("tg://proxy?server=p.example&port=443&secret=dd{SECRET}");
        let classic = format!("tg://proxy?secret={SECRET}&server=p.example&port=443");

        assert_eq!(extract_user_secret(&dd).as_deref(), Some(SECRET));
        assert_eq!(extract_user_secret(&classic).as_deref(), Some(SECRET));
    }

    #[test]
    fn extract_user_secret_normalizes_case() {
        let link =
            "tg://proxy?server=p.example&port=443&secret=EE0123456789ABCDEF0123456789ABCDEF00";

        assert_eq!(extract_user_secret(link).as_deref(), Some(SECRET));
    }

    #[test]
    fn extract_user_secret_rejects_malformed_links() {
        assert_eq!(
            extract_user_secret("tg://proxy?server=p.example&port=443"),
            None
        );
        assert_eq!(extract_user_secret("tg://proxy?secret=ee0123"), None);
        assert_eq!(
            extract_user_secret("tg://proxy?secret=eezz23456789abcdef0123456789abcdef00"),
            None
        );
        assert_eq!(
            extract_user_secret(&format!("tg://proxy?secret=ab{SECRET}")),
            None
        );
        assert_eq!(
            extract_user_secret(&format!("tg://proxy?secret=dd{SECRET}00")),
            None
        );
        assert_eq!(
            extract_user_secret(&format!("tg://proxy?secret=ee{SECRET}zz")),
            None
        );
        assert_eq!(
            extract_user_secret(&format!("tg://proxy?secret=ee{SECRET}")),
            None
        );
    }

    #[test]
    fn build_web_proxy_link_uses_dd_secret_by_default_mode() {
        assert_eq!(
            build_web_proxy_link("lk.example.com", SECRET, WebSecretMode::Dd),
            format!("tg://webproxy?server=lk.example.com&secret=dd{SECRET}")
        );
    }

    #[test]
    fn build_web_proxy_link_supports_plain_secret() {
        assert_eq!(
            build_web_proxy_link("lk.example.com", SECRET, WebSecretMode::Plain),
            format!("tg://webproxy?server=lk.example.com&secret={SECRET}")
        );
    }

    #[test]
    fn generate_user_secret_returns_32_hex_chars() {
        let secret = generate_user_secret();

        assert_eq!(secret.len(), 32);
        assert!(secret.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn build_fake_tls_secret_prefixes_domain_hex() {
        let secret = build_fake_tls_secret("0123456789abcdef0123456789abcdef", "example.com");

        assert_eq!(
            secret,
            "ee0123456789abcdef0123456789abcdef6578616d706c652e636f6d"
        );
    }

    #[test]
    fn build_proxy_link_uses_fake_tls_secret() {
        let params = TelemtLinkParams {
            host: "proxy.example.com".to_string(),
            port: 443,
            tls_domain: "example.com".to_string(),
        };

        let link = build_proxy_link(&params, "0123456789abcdef0123456789abcdef").unwrap();

        assert_eq!(
            link,
            "tg://proxy?server=proxy.example.com&port=443&secret=ee0123456789abcdef0123456789abcdef6578616d706c652e636f6d"
        );
    }
}

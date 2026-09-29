//! Проверка `initData` Telegram Mini App.
//!
//! Алгоритм Telegram: `secret_key = HMAC_SHA256(key = "WebAppData", msg = bot_token)`,
//! `hash = hex(HMAC_SHA256(key = secret_key, msg = data_check_string))`, где
//! `data_check_string` — все поля, кроме `hash`, отсортированные по ключу, в виде
//! `key=value`, соединённые `\n`.

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Допуск на расхождение часов для `auth_date` из будущего, секунд.
const CLOCK_SKEW_SECS: i64 = 60;

/// Пользователь Telegram из подписанного `initData`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WebAppUser {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

impl WebAppUser {
    /// Отображаемое имя: «Имя Фамилия» или `None`, если оба поля пусты.
    pub fn display_name(&self) -> Option<String> {
        let name = [self.first_name.as_deref(), self.last_name.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!name.is_empty()).then_some(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InitDataError {
    #[error("initData пуст")]
    Empty,
    #[error("в initData нет поля hash")]
    MissingHash,
    #[error("подпись initData не совпадает")]
    BadSignature,
    #[error("нет или некорректное поле auth_date")]
    BadAuthDate,
    #[error("initData устарел")]
    Expired,
    #[error("нет или некорректное поле user")]
    BadUser,
}

/// Проверяет подпись и свежесть `initData`, возвращает пользователя.
pub fn validate_init_data(
    init_data: &str,
    bot_token: &str,
    now: i64,
    max_age_secs: i64,
) -> Result<WebAppUser, InitDataError> {
    let init_data = init_data.trim();
    if init_data.is_empty() {
        return Err(InitDataError::Empty);
    }
    let fields: Vec<(String, String)> = form_urlencoded::parse(init_data.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let hash_hex = field(&fields, "hash").ok_or(InitDataError::MissingHash)?;
    let hash = hex::decode(hash_hex).map_err(|_| InitDataError::BadSignature)?;

    let secret_key = {
        let mut mac = HmacSha256::new_from_slice(b"WebAppData").expect("HMAC accepts any key size");
        mac.update(bot_token.as_bytes());
        mac.finalize().into_bytes()
    };
    let verified = signature_matches(&fields, &secret_key, &hash, &["hash"])
        || (field(&fields, "signature").is_some()
            && signature_matches(&fields, &secret_key, &hash, &["hash", "signature"]));
    if !verified {
        return Err(InitDataError::BadSignature);
    }

    let auth_date = field(&fields, "auth_date")
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(InitDataError::BadAuthDate)?;
    if auth_date > now + CLOCK_SKEW_SECS || now - auth_date > max_age_secs {
        return Err(InitDataError::Expired);
    }

    let user_json = field(&fields, "user").ok_or(InitDataError::BadUser)?;
    let user: WebAppUser = serde_json::from_str(user_json).map_err(|_| InitDataError::BadUser)?;
    if user.id <= 0 {
        return Err(InitDataError::BadUser);
    }
    Ok(user)
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

fn data_check_string(fields: &[(String, String)], exclude: &[&str]) -> String {
    let mut pairs: Vec<String> = fields
        .iter()
        .filter(|(key, _)| !exclude.contains(&key.as_str()))
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    pairs.sort();
    pairs.join("\n")
}

fn signature_matches(
    fields: &[(String, String)],
    secret_key: &[u8],
    expected: &[u8],
    exclude: &[&str],
) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret_key).expect("HMAC accepts any key size");
    mac.update(data_check_string(fields, exclude).as_bytes());
    mac.verify_slice(expected).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{InitDataError, WebAppUser, validate_init_data};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    const TOKEN: &str = "123456:TEST-token";
    const NOW: i64 = 1_790_000_000;
    const MAX_AGE: i64 = 86_400;
    const USER: &str = r#"{"id":42,"first_name":"Анна","last_name":"Иванова","username":"anna"}"#;

    fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    /// Строит initData, подписанный так, как это делает Telegram.
    fn signed(fields: &[(&str, &str)], token: &str, excluded_from_hash: &[&str]) -> String {
        let secret = hmac(b"WebAppData", token.as_bytes());
        let mut pairs: Vec<String> = fields
            .iter()
            .filter(|(key, _)| !excluded_from_hash.contains(key))
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        pairs.sort();
        let hash = hex::encode(hmac(&secret, pairs.join("\n").as_bytes()));
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            serializer.append_pair(key, value);
        }
        serializer.append_pair("hash", &hash);
        serializer.finish()
    }

    fn valid_fields(auth_date: &str) -> Vec<(&'static str, String)> {
        vec![
            ("query_id", "AAHdF6IQAAAAAN0XohDhrOrc".to_string()),
            ("user", USER.to_string()),
            ("auth_date", auth_date.to_string()),
        ]
    }

    fn as_refs<'a>(fields: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
        fields.iter().map(|(k, v)| (*k, v.as_str())).collect()
    }

    #[test]
    fn accepts_valid_init_data() {
        let fields = valid_fields(&NOW.to_string());
        let init = signed(&as_refs(&fields), TOKEN, &[]);

        let user = validate_init_data(&init, TOKEN, NOW + 10, MAX_AGE).unwrap();

        assert_eq!(
            user,
            WebAppUser {
                id: 42,
                first_name: Some("Анна".to_string()),
                last_name: Some("Иванова".to_string()),
                username: Some("anna".to_string()),
            }
        );
        assert_eq!(user.display_name().as_deref(), Some("Анна Иванова"));
    }

    #[test]
    fn accepts_signature_field_in_either_hashing_mode() {
        let mut fields = valid_fields(&NOW.to_string());
        fields.push(("signature", "abc".to_string()));
        let included = signed(&as_refs(&fields), TOKEN, &[]);
        let excluded = signed(&as_refs(&fields), TOKEN, &["signature"]);

        assert!(validate_init_data(&included, TOKEN, NOW, MAX_AGE).is_ok());
        assert!(validate_init_data(&excluded, TOKEN, NOW, MAX_AGE).is_ok());
    }

    #[test]
    fn rejects_tampered_field() {
        let fields = valid_fields(&NOW.to_string());
        let init = signed(&as_refs(&fields), TOKEN, &[]).replace("%22id%22%3A42", "%22id%22%3A43");

        assert_eq!(
            validate_init_data(&init, TOKEN, NOW, MAX_AGE),
            Err(InitDataError::BadSignature)
        );
    }

    #[test]
    fn rejects_other_bot_token() {
        let fields = valid_fields(&NOW.to_string());
        let init = signed(&as_refs(&fields), "999:OTHER", &[]);

        assert_eq!(
            validate_init_data(&init, TOKEN, NOW, MAX_AGE),
            Err(InitDataError::BadSignature)
        );
    }

    #[test]
    fn rejects_expired_and_future_auth_date() {
        let old = valid_fields(&(NOW - MAX_AGE - 1).to_string());
        let future = valid_fields(&(NOW + 3_600).to_string());

        assert_eq!(
            validate_init_data(&signed(&as_refs(&old), TOKEN, &[]), TOKEN, NOW, MAX_AGE),
            Err(InitDataError::Expired)
        );
        assert_eq!(
            validate_init_data(&signed(&as_refs(&future), TOKEN, &[]), TOKEN, NOW, MAX_AGE),
            Err(InitDataError::Expired)
        );
    }

    #[test]
    fn rejects_missing_parts() {
        assert_eq!(
            validate_init_data("  ", TOKEN, NOW, MAX_AGE),
            Err(InitDataError::Empty)
        );
        assert_eq!(
            validate_init_data("auth_date=1&user=%7B%7D", TOKEN, NOW, MAX_AGE),
            Err(InitDataError::MissingHash)
        );
        assert_eq!(
            validate_init_data("auth_date=1&hash=zz", TOKEN, NOW, MAX_AGE),
            Err(InitDataError::BadSignature)
        );

        let no_user = [("auth_date", NOW.to_string())];
        assert_eq!(
            validate_init_data(&signed(&as_refs(&no_user), TOKEN, &[]), TOKEN, NOW, MAX_AGE),
            Err(InitDataError::BadUser)
        );
    }
}

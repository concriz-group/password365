//! Messages exchanged with the browser extension (JSON).

use password365_app::AppError;
use password365_app::browser::SubmittedLogin;
use password365_app::dto::GenerateRequest;
use password365_crypto::secret::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A request from the extension. `id` is echoed back in the response.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Correlation id chosen by the extension.
    pub id: u64,
    /// What to do.
    #[serde(flatten)]
    pub body: RequestBody,
}

/// The operations available to the extension.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    /// Is the app set up / unlocked?
    Status,
    /// Unlock with the master password (the Secret Key comes from this device).
    Unlock {
        /// Master password (zeroized on drop, redacted in Debug).
        password: SecretString,
    },
    /// Logins for the current page (no secrets).
    LoginsForUrl {
        /// Page URL.
        page_url: String,
    },
    /// Username + password of one login, for filling `page_url`.
    Fill {
        /// Item id (named `item_id` so it does not clash with the envelope `id`).
        item_id: String,
        /// Page URL (re-checked against the item's websites).
        page_url: String,
    },
    /// After a form submit: new / known / changed?
    CheckSubmitted {
        /// The submitted form.
        login: SubmittedLogin,
    },
    /// Save (or update) a submitted login.
    SaveSubmitted {
        /// The submitted form.
        login: SubmittedLogin,
        /// Target vault (default: Personal).
        #[serde(default)]
        vault_id: Option<String>,
    },
    /// Generate a password for a sign-up form.
    Generate {
        /// Recipe.
        request: GenerateRequest,
    },
    /// The default identity and card (titles, last four digits; no secrets).
    Profile,
    /// The default identity's or card's values for a form on `page_url`
    /// (cards only on https pages).
    ProfileFill {
        /// Identity or card.
        profile: password365_app::profile::ProfileKind,
        /// Another identity or card than the default (`None`: the default).
        #[serde(default)]
        item_id: Option<String>,
        /// Page URL.
        page_url: String,
    },
    /// Create a passkey for the page (after the user chose to save it).
    PasskeyRegister {
        /// Page URL (the tab's, from the browser; checked against the rp id).
        page_url: String,
        /// Title for a new login (the site's name).
        #[serde(default)]
        title: String,
        /// What the page asked for.
        request: password365_webauthn::authenticator::CreateRequest,
        /// Credential ids the site already has for this account.
        #[serde(default)]
        exclude: Vec<String>,
        /// The master password, typed when the site requires verification.
        #[serde(default)]
        password: Option<SecretString>,
    },
    /// Logins whose passkey the page may use (no secrets).
    PasskeyOptions {
        /// Page URL.
        page_url: String,
        /// Relying-party id the page asks for.
        rp_id: String,
        /// Credential ids the site accepts (empty: any).
        #[serde(default)]
        allow: Vec<String>,
    },
    /// Sign in to the page with a login's passkey (after the user chose it).
    PasskeyAssert {
        /// Item id.
        item_id: String,
        /// Page URL.
        page_url: String,
        /// The page's challenge (base64url).
        challenge: String,
        /// The page's `userVerification`.
        #[serde(default)]
        user_verification: password365_webauthn::authenticator::UserVerification,
        /// The master password, typed when the site requires verification.
        #[serde(default)]
        password: Option<SecretString>,
    },
}

/// Error body sent to the extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Machine-readable code (`locked`, `validation`, `bad_request`, …).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

impl From<&AppError> for ErrorBody {
    fn from(e: &AppError) -> Self {
        Self {
            code: e.code().to_owned(),
            message: e.to_string(),
        }
    }
}

/// A response to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The request's id (0 if the request could not be parsed).
    pub id: u64,
    /// `true` on success.
    pub ok: bool,
    /// Result on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    /// Successful response.
    #[must_use]
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    /// Failed response.
    #[must_use]
    pub fn failure(id: u64, code: &str, message: &str) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(ErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

/// Parses a request; malformed input yields a ready-to-send error response.
pub fn parse_request(bytes: &[u8]) -> Result<Request, Response> {
    serde_json::from_slice(bytes).map_err(|e| {
        let id = serde_json::from_slice::<Value>(bytes)
            .ok()
            .and_then(|v| v.get("id").and_then(Value::as_u64))
            .unwrap_or(0);
        Response::failure(id, "bad_request", &format!("malformed request: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_request_type() {
        let cases = [
            r#"{"id":1,"type":"status"}"#,
            r#"{"id":2,"type":"logins_for_url","page_url":"https://a.com"}"#,
            r#"{"id":3,"type":"fill","item_id":"x","page_url":"https://a.com"}"#,
            r#"{"id":4,"type":"check_submitted","login":{"page_url":"https://a.com","password":"p"}}"#,
            r#"{"id":5,"type":"save_submitted","login":{"page_url":"https://a.com","password":"p"}}"#,
            r#"{"id":6,"type":"generate","request":{"type":"pin","digits":6}}"#,
        ];
        for raw in cases {
            assert!(parse_request(raw.as_bytes()).is_ok(), "{raw}");
        }
    }

    #[test]
    fn malformed_requests_become_bad_request_responses() {
        let r = parse_request(br#"{"id":9,"type":"nope"}"#).unwrap_err();
        assert_eq!((r.id, r.ok), (9, false));
        assert_eq!(r.error.unwrap().code, "bad_request");
        assert_eq!(parse_request(b"not json").unwrap_err().id, 0);
    }

    #[test]
    fn response_json_shape() {
        let ok = serde_json::to_string(&Response::success(1, serde_json::json!([1]))).unwrap();
        assert_eq!(ok, r#"{"id":1,"ok":true,"result":[1]}"#);
        let err =
            serde_json::to_string(&Response::failure(2, "locked", "the vault is locked")).unwrap();
        assert_eq!(
            err,
            r#"{"id":2,"ok":false,"error":{"code":"locked","message":"the vault is locked"}}"#
        );
    }

    #[test]
    fn error_body_from_app_error() {
        let body = ErrorBody::from(&AppError::Locked);
        assert_eq!(body.code, "locked");
    }
}

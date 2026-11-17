//! Executes extension requests against the application service.

use password365_app::{AppError, AppService};
use serde::Serialize;
use serde_json::Value;

use crate::protocol::{ErrorBody, Request, RequestBody, Response, parse_request};

/// Handles one framed request (raw JSON bytes) and returns the JSON response.
pub fn handle_bytes(service: &mut AppService, bytes: &[u8]) -> Vec<u8> {
    let response = match parse_request(bytes) {
        Ok(request) => handle(service, request),
        Err(response) => response,
    };
    serde_json::to_vec(&response).unwrap_or_else(|_| br#"{"id":0,"ok":false}"#.to_vec())
}

/// Executes a parsed request.
pub fn handle(service: &mut AppService, request: Request) -> Response {
    let id = request.id;
    let result = match request.body {
        RequestBody::Status => to_value(Ok(service.status())),
        RequestBody::Unlock { password } => to_value(service.unlock(password, None)),
        RequestBody::LoginsForUrl { page_url } => to_value(service.logins_for_url(&page_url)),
        RequestBody::Fill { item_id, page_url } => {
            to_value(service.fill_credentials(&item_id, &page_url))
        }
        RequestBody::CheckSubmitted { login } => to_value(service.check_submitted(&login)),
        RequestBody::SaveSubmitted { login, vault_id } => {
            to_value(service.save_submitted(&login, vault_id.as_deref()))
        }
        RequestBody::Generate { request } => to_value(service.generate(&request)),
        RequestBody::Profile => to_value(service.profile()),
        RequestBody::ProfileFill {
            profile,
            item_id,
            page_url,
        } => to_value(service.profile_form_values_of(profile, item_id.as_deref(), &page_url)),
        RequestBody::PasskeyRegister {
            page_url,
            title,
            request,
            exclude,
            password,
        } => to_value(
            service
                .passkey_register(&page_url, &title, &request, &exclude, None, password)
                .map(|(item_id, registration)| serde_json::json!({"item_id": item_id, "registration": registration})),
        ),
        RequestBody::PasskeyOptions {
            page_url,
            rp_id,
            allow,
        } => to_value(service.passkey_options(&page_url, &rp_id, &allow)),
        RequestBody::PasskeyAssert {
            item_id,
            page_url,
            challenge,
            user_verification,
            password,
        } => to_value(service.passkey_assert(
            &page_url,
            &item_id,
            &challenge,
            user_verification,
            password,
        )),
    };
    match result {
        Ok(value) => Response::success(id, value),
        Err(error) => Response {
            id,
            ok: false,
            result: None,
            error: Some(error),
        },
    }
}

fn to_value<T: Serialize>(result: Result<T, AppError>) -> Result<Value, ErrorBody> {
    let value = result.map_err(|e| ErrorBody::from(&e))?;
    serde_json::to_value(value).map_err(|e| ErrorBody {
        code: "internal".into(),
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use password365_app::AppConfig;
    use password365_app::item_input::ItemInput;
    use password365_app::secret_key_store::MemorySecretKeyStore;
    use password365_crypto::kdf::KdfParams;
    use serde_json::json;

    use super::*;

    fn service() -> (tempfile::TempDir, AppService) {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig {
            store_path: dir.path().join("s"),
            kdf: KdfParams::insecure_for_tests(),
            auto_lock_ms: 0,
            settings_path: None,
        };
        let mut app = AppService::new(config, Box::new(MemorySecretKeyStore::default()));
        let pw = "long enough password";
        app.create_account(pw.into(), &pw.into()).unwrap();
        (dir, app)
    }

    fn call(app: &mut AppService, request: &serde_json::Value) -> serde_json::Value {
        let out = handle_bytes(app, &serde_json::to_vec(request).unwrap());
        serde_json::from_slice(&out).unwrap()
    }

    #[test]
    fn passkeys_over_the_protocol() {
        let (_d, mut app) = service();
        let request = json!({"rp_id": "example.com", "user_id": "dXNlcg", "user_name": "ana",
                             "challenge": "Y2g", "algorithms": [-7]});
        let r = call(
            &mut app,
            &json!({"id": 1, "type": "passkey_register", "page_url": "https://example.com/join",
                    "title": "Example", "request": request}),
        );
        let credential = r["result"]["registration"]["credential_id"].clone();
        assert!(credential.is_string(), "{r}");
        let r = call(
            &mut app,
            &json!({"id": 2, "type": "passkey_options", "page_url": "https://example.com", "rp_id": "example.com"}),
        );
        assert_eq!(r["result"][0]["user_name"], "ana");
        assert!(
            r["result"][0].get("key").is_none(),
            "no secrets in the list"
        );
        let item = r["result"][0]["item_id"].clone();
        let r = call(
            &mut app,
            &json!({"id": 3, "type": "passkey_assert", "item_id": item, "page_url": "https://example.com", "challenge": "Y2g"}),
        );
        assert_eq!(r["result"]["credential_id"], credential);
        let r = call(
            &mut app,
            &json!({"id": 4, "type": "passkey_assert", "item_id": item, "page_url": "https://evil.com", "challenge": "Y2g"}),
        );
        assert_eq!(
            r["error"]["code"], "validation",
            "another site cannot use it"
        );
    }

    #[test]
    fn status_and_generate() {
        let (_d, mut app) = service();
        let r = call(&mut app, &json!({"id": 1, "type": "status"}));
        assert_eq!(
            r,
            json!({"id": 1, "ok": true, "result": {"initialized": true, "unlocked": true}})
        );
        let r = call(
            &mut app,
            &json!({"id": 2, "type": "generate", "request": {"type": "pin", "digits": 4}}),
        );
        assert_eq!(r["result"]["value"].as_str().unwrap().len(), 4);
    }

    #[test]
    fn login_flow_over_the_protocol() {
        let (_d, mut app) = service();
        let item = app
            .create_item(&ItemInput {
                title: "GitHub".into(),
                username: Some("octo".into()),
                urls: vec!["https://github.com".into()],
                password: Some("pw".into()),
                ..ItemInput::default()
            })
            .unwrap();
        let r = call(
            &mut app,
            &json!({"id": 3, "type": "logins_for_url", "page_url": "https://github.com/login"}),
        );
        assert_eq!(r["result"][0]["title"], "GitHub");
        assert!(
            r["result"][0].get("password").is_none(),
            "listing never carries secrets"
        );

        let r = call(
            &mut app,
            &json!({"id": 4, "type": "fill", "item_id": item.summary.id, "page_url": "https://github.com"}),
        );
        assert_eq!(r["result"], json!({"username": "octo", "password": "pw"}));

        let login =
            json!({"page_url": "https://github.com", "username": "octo", "password": "pw2"});
        let r = call(
            &mut app,
            &json!({"id": 5, "type": "check_submitted", "login": login}),
        );
        assert_eq!(r["result"]["status"], "password_changed");
        let r = call(
            &mut app,
            &json!({"id": 6, "type": "save_submitted", "login": login}),
        );
        assert_eq!(r["result"]["password_history_count"], 1);
    }

    #[test]
    fn profile_over_the_protocol() {
        let (_d, mut app) = service();
        app.create_item(&ItemInput {
            kind: Some("card".into()),
            title: "Visa".into(),
            fields: Some(vec![password365_app::item_input::FieldValue {
                label: "number".into(),
                value: "4111111111114242".into(),
                ..Default::default()
            }]),
            ..ItemInput::default()
        })
        .unwrap();
        let r = call(&mut app, &json!({"id": 20, "type": "profile"}));
        assert_eq!(r["result"]["card"]["last4"], "4242");
        assert_eq!(r["result"]["identity"], Value::Null);
        let r = call(
            &mut app,
            &json!({"id": 21, "type": "profile_fill", "profile": "card", "page_url": "https://shop.example"}),
        );
        assert_eq!(r["result"]["cc-number"], "4111111111114242");
        let r = call(
            &mut app,
            &json!({"id": 22, "type": "profile_fill", "profile": "card", "page_url": "http://shop.example"}),
        );
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn unlock_with_the_master_password() {
        let (_d, mut app) = service();
        app.lock();
        let r = call(
            &mut app,
            &json!({"id": 10, "type": "unlock", "password": "wrong password!"}),
        );
        assert_eq!(r["ok"], false);
        assert!(!app.status().unlocked);
        let r = call(
            &mut app,
            &json!({"id": 11, "type": "unlock", "password": "long enough password"}),
        );
        assert_eq!(r, json!({"id": 11, "ok": true, "result": null}));
        assert!(app.status().unlocked);
    }

    #[test]
    fn errors_carry_codes() {
        let (_d, mut app) = service();
        app.lock();
        let r = call(
            &mut app,
            &json!({"id": 7, "type": "logins_for_url", "page_url": "https://a.com"}),
        );
        assert_eq!(r["ok"], false);
        assert_eq!(r["error"]["code"], "locked");
        let out: serde_json::Value = serde_json::from_slice(&handle_bytes(&mut app, b"{")).unwrap();
        assert_eq!(out["error"]["code"], "bad_request");
    }

    #[test]
    fn save_with_vault_and_fill_mismatch() {
        let (_d, mut app) = service();
        let vault = app.create_vault("Work").unwrap();
        let login = json!({"page_url": "https://a.com", "username": "u", "password": "p"});
        let r = call(
            &mut app,
            &json!({"id": 8, "type": "save_submitted", "login": login, "vault_id": vault.id}),
        );
        assert_eq!(r["result"]["vault_id"], vault.id);
        let id = r["result"]["id"].as_str().unwrap().to_owned();
        let r = call(
            &mut app,
            &json!({"id": 9, "type": "fill", "item_id": id, "page_url": "https://b.com"}),
        );
        assert_eq!(r["error"]["code"], "validation");
    }
}

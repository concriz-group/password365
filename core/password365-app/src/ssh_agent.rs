//! Which SSH Key items the SSH agent can offer (Settings › Developer).

use password365_model::{CustomField, ItemDetails, ItemKind};
use password365_ssh::{AgentKey, KeyError, load_key};
use serde::Serialize;

use crate::{AppService, Result};

/// Field holding the private key (the SSH Key template's).
const PRIVATE_KEY: &str = "private key";
/// Fields that may hold the key's passphrase.
const PASSPHRASE: [&str; 2] = ["passphrase", "password"];

/// An SSH Key item's details with the public key, fingerprint and key type
/// worked out from its private key (when it is an Ed25519 key the app can
/// read); other items and keys come back unchanged.
#[must_use]
pub fn with_derived(kind: ItemKind, mut details: ItemDetails) -> ItemDetails {
    if kind != ItemKind::SshKey {
        return details;
    }
    let field = |labels: &[&str]| {
        details
            .custom_fields
            .iter()
            .find(|f| labels.iter().any(|l| f.label.eq_ignore_ascii_case(l)))
            .map(|f| f.value.expose().to_owned())
    };
    let Some(pem) = field(&[PRIVATE_KEY]) else {
        return details;
    };
    let passphrase = field(&PASSPHRASE);
    let Ok(key) = load_key("", "", &pem, passphrase.as_deref()) else {
        return details;
    };
    for (label, value) in [
        ("public key", key.public_openssh()),
        ("fingerprint", key.fingerprint()),
        ("key type", "Ed25519".to_owned()),
    ] {
        details
            .custom_fields
            .retain(|f| !f.label.eq_ignore_ascii_case(label));
        details.custom_fields.push(CustomField {
            label: label.to_owned(),
            value: value.into(),
            concealed: false,
        });
    }
    details
}

/// An SSH Key item as Settings › Developer lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SshKeyDto {
    /// Item id.
    pub id: String,
    /// Item title.
    pub title: String,
    /// `SHA256:…` when the agent can use the key.
    pub fingerprint: Option<String>,
    /// Why the agent cannot use it: `unreadable` or `encrypted`.
    pub problem: Option<String>,
}

/// An SSH Key item's id, title, and its key or why it cannot be used.
type Loaded = (String, String, std::result::Result<AgentKey, KeyError>);

impl AppService {
    /// Every active SSH Key item with its private key loaded, or why it cannot be.
    fn ssh_key_results(&mut self) -> Result<Vec<Loaded>> {
        let ids: Vec<String> = self
            .session()?
            .items()
            .into_iter()
            .filter(|s| s.kind == ItemKind::SshKey && crate::service::active(s))
            .map(|s| s.id.to_string())
            .collect();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let item = self.load_item(&id)?;
            let field = |labels: &[&str]| {
                item.details
                    .custom_fields
                    .iter()
                    .find(|f| labels.iter().any(|l| f.label.eq_ignore_ascii_case(l)))
                    .map(|f| f.value.expose().to_owned())
            };
            let passphrase = field(&PASSPHRASE).or_else(|| {
                item.details
                    .password
                    .as_ref()
                    .map(|p| p.expose().to_owned())
            });
            let title = item.overview.title.clone();
            let loaded = field(&[PRIVATE_KEY])
                .ok_or(KeyError::Unreadable)
                .and_then(|pem| load_key(&id, &title, &pem, passphrase.as_deref()));
            out.push((id, title, loaded));
        }
        Ok(out)
    }

    /// The keys the agent offers (empty when the agent is off).
    pub fn ssh_agent_keys(&mut self) -> Result<Vec<AgentKey>> {
        if !self.settings().ssh_agent {
            return Ok(Vec::new());
        }
        Ok(self
            .ssh_key_results()?
            .into_iter()
            .filter_map(|(_, _, k)| k.ok())
            .collect())
    }

    /// Every SSH Key item and whether the agent can use it.
    pub fn ssh_keys(&mut self) -> Result<Vec<SshKeyDto>> {
        Ok(self
            .ssh_key_results()?
            .into_iter()
            .map(|(id, title, loaded)| match loaded {
                Ok(key) => SshKeyDto {
                    id,
                    title,
                    fingerprint: Some(key.fingerprint()),
                    problem: None,
                },
                Err(e) => SshKeyDto {
                    id,
                    title,
                    fingerprint: None,
                    problem: Some(
                        match e {
                            KeyError::Encrypted => "encrypted",
                            KeyError::Unreadable | KeyError::Unsupported => "unreadable",
                        }
                        .into(),
                    ),
                },
            })
            .collect())
    }
}

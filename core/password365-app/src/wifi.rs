//! Joining a Wi-Fi network by scanning a QR code (Wireless Router items), in
//! the `WIFI:` format phones understand.

use password365_model::{Item, ItemKind};

use crate::{AppService, Result};

/// `WIFI:` escapes `\ ; , : "` with a backslash.
fn escape(text: &str) -> String {
    text.chars()
        .fold(String::with_capacity(text.len()), |mut out, c| {
            if matches!(c, '\\' | ';' | ',' | ':' | '"') {
                out.push('\\');
            }
            out.push(c);
            out
        })
}

/// The `WIFI:` text of a Wireless Router item (`None` without a network name).
#[must_use]
pub fn join_text(item: &Item) -> Option<String> {
    if item.kind != ItemKind::WirelessRouter {
        return None;
    }
    let field = |label: &str| {
        item.details
            .custom_fields
            .iter()
            .find(|f| f.label.eq_ignore_ascii_case(label))
            .map(|f| f.value.expose().to_owned())
            .filter(|v| !v.is_empty())
    };
    let ssid = field("network name")?;
    let password = field("wireless network password");
    let security = field("wireless security")
        .unwrap_or_default()
        .to_uppercase();
    let kind = match (password.is_some(), security.as_str()) {
        (false, _) => "nopass",
        (true, s) if s.contains("WEP") => "WEP",
        _ => "WPA",
    };
    let password = password
        .map(|p| format!("P:{};", escape(&p)))
        .unwrap_or_default();
    Some(format!("WIFI:T:{kind};S:{};{password};", escape(&ssid)))
}

impl AppService {
    /// The `WIFI:` text to show as a QR code for item `id`, if it is a network.
    pub fn wifi_join_text(&mut self, id: &str) -> Result<Option<String>> {
        Ok(join_text(&self.load_item(id)?))
    }
}

#[cfg(test)]
mod tests {
    use password365_model::{CustomField, ItemDetails, ItemOverview, Timestamp, VaultId};

    use super::*;

    fn router(fields: &[(&str, &str)]) -> Item {
        let details = ItemDetails {
            custom_fields: fields
                .iter()
                .map(|(l, v)| CustomField {
                    label: (*l).into(),
                    value: (*v).into(),
                    concealed: false,
                })
                .collect(),
            ..ItemDetails::default()
        };
        Item::new(
            VaultId::new(),
            ItemKind::WirelessRouter,
            ItemOverview::new("Home").unwrap(),
            details,
            Timestamp::now(),
        )
    }

    #[test]
    fn builds_the_wifi_text() {
        assert_eq!(
            join_text(&router(&[
                ("network name", "My;Net"),
                ("wireless network password", "p:w\"")
            ]))
            .as_deref(),
            Some(r#"WIFI:T:WPA;S:My\;Net;P:p\:w\";;"#)
        );
        assert_eq!(
            join_text(&router(&[
                ("network name", "Old"),
                ("wireless network password", "x"),
                ("wireless security", "wep")
            ]))
            .as_deref(),
            Some("WIFI:T:WEP;S:Old;P:x;;")
        );
        assert_eq!(
            join_text(&router(&[("network name", "Cafe")])).as_deref(),
            Some("WIFI:T:nopass;S:Cafe;;")
        );
        assert_eq!(join_text(&router(&[])), None);
        let mut not_router = router(&[("network name", "x")]);
        not_router.kind = ItemKind::Login;
        assert_eq!(join_text(&not_router), None);
    }
}

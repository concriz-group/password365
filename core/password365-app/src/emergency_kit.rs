//! The Emergency Kit as a PDF kept inside the vault (a Document item with the file attached),
//! like 1Password: the Secret Key is readable from any unlocked device, with a preview.
//! The PDF is written here by hand (one A4 page, built-in fonts), so it needs no library.

use std::fmt::Write as _;

use password365_crypto::secret::SecretString;
use password365_model::{ItemKind, ItemOverview};
use serde::Deserialize;

use crate::service::parse_id;
use crate::{AppService, Result};

/// The sheet's words, already in the interface language (the app owns the translations).
#[derive(Debug, Clone, Deserialize)]
pub struct KitText {
    /// Title of the sheet (also the item's title).
    pub title: String,
    /// "Created on …".
    pub created: String,
    /// Why to keep it safe.
    pub keep: String,
    /// "Secret Key".
    pub secret_key_label: String,
    /// "Master password".
    pub master_password_label: String,
    /// Hint beside the empty box.
    pub write_here: String,
    /// "How to use this sheet".
    pub how_to: String,
    /// The numbered steps.
    pub steps: Vec<String>,
}

/// File name of the attachment.
pub const KIT_FILE: &str = "Emergency Kit.pdf";

/// `(` `)` `\` escaped; characters outside Latin-1 become `?` (`WinAnsi` fonts).
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 32 => out.push(' '),
            c if (c as u32) < 127 => out.push(c),
            c if (c as u32) <= 255 => {
                let _ = write!(out, "\\{:03o}", c as u32);
            }
            _ => out.push('?'),
        }
    }
    out
}

/// Breaks `text` into lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

struct Page {
    ops: String,
    y: f32,
}

impl Page {
    fn text(&mut self, font: &str, size: f32, text: &str, gap: f32) {
        self.y -= size + gap;
        let _ = writeln!(
            self.ops,
            "BT /{font} {size} Tf 56 {:.1} Td ({}) Tj ET",
            self.y,
            escape(text)
        );
    }

    fn paragraph(&mut self, font: &str, size: f32, text: &str) {
        for line in wrap(text, 88) {
            self.text(font, size, &line, 3.0);
        }
        self.y -= 6.0;
    }
}

/// The PDF bytes of the kit for `secret_key`.
#[must_use]
pub fn kit_pdf(secret_key: &str, text: &KitText) -> Vec<u8> {
    let mut page = Page {
        ops: String::new(),
        y: 800.0,
    };
    page.text("F2", 22.0, &text.title, 4.0);
    page.text("F1", 10.0, &text.created, 4.0);
    page.y -= 8.0;
    page.paragraph("F1", 11.0, &text.keep);
    page.y -= 6.0;
    page.text("F2", 13.0, &text.secret_key_label, 6.0);
    page.text("F3", 16.0, secret_key, 6.0);
    page.y -= 14.0;
    page.text("F2", 13.0, &text.master_password_label, 6.0);
    page.y -= 8.0;
    let _ = writeln!(page.ops, "56 {:.1} 483 34 re S", page.y - 34.0);
    page.y -= 34.0;
    page.text("F1", 9.0, &text.write_here, 6.0);
    page.y -= 14.0;
    page.text("F2", 13.0, &text.how_to, 6.0);
    for (i, step) in text.steps.iter().enumerate() {
        page.paragraph("F1", 11.0, &format!("{}. {step}", i + 1));
    }

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R /F2 6 0 R /F3 7 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            page.ops.len(),
            page.ops
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>"
            .to_owned(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).into_bytes());
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for offset in offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    pdf.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .into_bytes(),
    );
    pdf
}

impl AppService {
    /// Keeps the Emergency Kit PDF in the vault as a Document item (once: when the vault already
    /// has one with this title, its id is returned). Needs the master password again.
    pub fn save_emergency_kit(&mut self, password: SecretString, text: &KitText) -> Result<String> {
        let secret_key = self.reveal_secret_key(password)?;
        let existing = self
            .session()?
            .items()
            .into_iter()
            .find(|s| s.kind == ItemKind::Document && s.overview.title == text.title)
            .map(|s| s.id.to_string());
        if let Some(id) = existing {
            return Ok(id);
        }
        let vault = self.vault_or_default(None)?;
        let id = self.session()?.add_item(
            vault,
            ItemKind::Document,
            ItemOverview::new(&text.title)?,
            password365_model::ItemDetails::default(),
        )?;
        let pdf = kit_pdf(&secret_key, text);
        self.session()?
            .attach(id, KIT_FILE, "application/pdf", &pdf)?;
        let _ = parse_id::<password365_model::ItemId>(&id.to_string())?;
        Ok(id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::tests::with_account;

    fn text() -> KitText {
        KitText {
            title: "Emergency Kit".into(),
            created: "Created on 1 Jan".into(),
            keep: "Keep this (safe) sheet. Atenção: não perca.".into(),
            secret_key_label: "Secret Key".into(),
            master_password_label: "Master password".into(),
            write_here: "Write it here".into(),
            how_to: "How to".into(),
            steps: vec!["One".into(), "Two".into()],
        }
    }

    #[test]
    fn the_pdf_is_well_formed_and_holds_the_key() {
        let pdf = kit_pdf("A3-ABCDEF-GHJKLM", &text());
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4") && s.trim_end().ends_with("%%EOF"));
        assert!(s.contains("(A3-ABCDEF-GHJKLM) Tj"));
        assert!(s.contains(r"\(safe\)") && s.contains(r"Aten\347\343o"));
        let xref = s.rfind("startxref\n").unwrap();
        let offset: usize = s[xref + 10..].lines().next().unwrap().parse().unwrap();
        assert!(s[offset..].starts_with("xref"));
    }

    #[test]
    fn long_text_wraps() {
        assert_eq!(wrap("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
    }

    #[test]
    fn saving_needs_the_password_and_happens_once() {
        let mut fx = with_account();
        assert!(
            fx.app
                .save_emergency_kit("wrong password".to_owned().into(), &text())
                .is_err()
        );
        let password: SecretString = crate::service::tests::PW.into();
        let id = fx
            .app
            .save_emergency_kit(crate::service::tests::PW.into(), &text())
            .unwrap();
        let again = fx.app.save_emergency_kit(password, &text()).unwrap();
        assert_eq!(id, again);
        let item = fx.app.item(&id).unwrap();
        assert_eq!(item.attachments.len(), 1);
        assert_eq!(item.attachments[0].content_type, "application/pdf");
        let bytes = fx
            .app
            .attachment_bytes(&id, &item.attachments[0].id)
            .unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}

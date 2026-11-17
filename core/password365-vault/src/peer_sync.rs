//! One sync conversation with a paired device, independent of the transport.
//!
//! Both sides run the same steps:
//! 1. send `Hello` (my id, how far I got with your changes);
//! 2. on the peer's `Hello`, send the devices removed from the account
//!    (`Revoked`) and the latest master password change (`Password`), both
//!    merged by the peer, then my changes since then, then
//!    `Done{my seq}`;
//! 3. on `Done`, merge everything received in one transaction. If merging
//!    produced new local changes (a combined state), send them plus a new
//!    `Done` *before* answering `Ack`, so the peer sees them in this same
//!    conversation;
//! 4. on `Ack`, record what the peer now has;
//! 5. after merging (step 3), ask for the bytes of attachments the items now
//!    name but this device lacks (`WantFiles`, sent before the `Ack`); the
//!    peer answers each with `FilePart`s or `NoFile`. Received bytes are kept
//!    only if an item names them and they match its digest.
//!
//! The conversation is finished once I have handled a `Done`, have nothing
//! left to merge, every file I asked for has arrived (or the peer has none),
//! and the peer acknowledged my latest `Done`. Merges are idempotent, so the
//! echo round always ends.

use std::collections::{BTreeMap, BTreeSet};

use password365_model::attachment::MAX_ATTACHMENT_BYTES;
use password365_model::{AttachmentId, Timestamp};
use password365_sync::{DeviceId, ItemState};

use crate::session_sync::ApplyReport;
use crate::wire::{Message, chunk, file_parts};
use crate::{Result, Session, VaultError};

/// A sync in progress with one peer.
#[derive(Debug)]
pub struct PeerSync {
    peer: DeviceId,
    max_message: usize,
    hello_received: bool,
    inbox: Vec<ItemState>,
    pieces: crate::wire::ItemPieces,
    last_sent_done: Option<u64>,
    acked_by_peer: Option<u64>,
    done_handled: bool,
    report: ApplyReport,
    /// Attachments asked for and not received yet.
    wanted: BTreeSet<AttachmentId>,
    /// Pieces received so far, by attachment.
    receiving: BTreeMap<AttachmentId, Vec<u8>>,
}

/// Largest encrypted attachment accepted (the largest file, padded, plus
/// encryption overhead).
const MAX_SEALED: u64 = MAX_ATTACHMENT_BYTES + MAX_ATTACHMENT_BYTES / 8 + 1024;

impl PeerSync {
    /// Starts syncing with the paired device whose channel key is
    /// `remote_static`. Fails if no paired device has that key.
    /// `max_message` is the transport's plaintext limit.
    pub fn start(
        session: &Session,
        remote_static: &[u8; 32],
        max_message: usize,
    ) -> Result<(Self, Message)> {
        let peer = session
            .peers()
            .into_iter()
            .find(|p| &p.static_public == remote_static)
            .ok_or(VaultError::UnknownDevice)?;
        let hello = Message::Hello {
            device_id: session.device_id(),
            have_from_you: peer.received_seq,
        };
        let sync = Self {
            peer: peer.device_id,
            max_message,
            hello_received: false,
            inbox: Vec::new(),
            pieces: crate::wire::ItemPieces::default(),
            last_sent_done: None,
            acked_by_peer: None,
            done_handled: false,
            report: ApplyReport::default(),
            wanted: BTreeSet::new(),
            receiving: BTreeMap::new(),
        };
        Ok((sync, hello))
    }

    /// The device being synced with.
    #[must_use]
    pub const fn peer(&self) -> DeviceId {
        self.peer
    }

    /// `true` when both sides have everything.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.done_handled
            && self.inbox.is_empty()
            && self.wanted.is_empty()
            && self.last_sent_done.is_some()
            && self.acked_by_peer == self.last_sent_done
    }

    /// What merging the peer's changes did here (accumulated).
    #[must_use]
    pub fn report(&self) -> &ApplyReport {
        &self.report
    }

    /// Handles one message from the peer; returns the messages to send back.
    pub fn handle(&mut self, session: &mut Session, message: Message) -> Result<Vec<Message>> {
        match message {
            items @ (Message::Items(_) | Message::ItemsV2(_) | Message::ItemsV1(_)) => {
                self.require_hello()?;
                self.inbox.extend(items.into_states().unwrap_or_default());
                Ok(Vec::new())
            }
            Message::ItemPart { last, bytes } => {
                self.require_hello()?;
                self.inbox.extend(self.pieces.add(last, &bytes)?);
                Ok(Vec::new())
            }
            Message::Hello {
                device_id,
                have_from_you,
            } => {
                if device_id != self.peer || self.hello_received {
                    return Err(VaultError::SyncProtocol("unexpected hello"));
                }
                self.hello_received = true;
                let mut out = vec![Message::Revoked(session.revoked_devices())];
                if let Some(change) = session.password_change() {
                    out.push(Message::Password(change.clone()));
                }
                out.extend(self.send_changes(session, have_from_you)?);
                Ok(out)
            }
            Message::Revoked(ids) => {
                self.require_hello()?;
                if ids.contains(&self.peer) {
                    return Err(VaultError::RevokedDevice);
                }
                session.merge_revoked_from(self.peer, &ids)?;
                Ok(Vec::new())
            }
            Message::Password(change) => {
                self.require_hello()?;
                self.report.password_changed |= session.merge_password_change(change)?;
                Ok(Vec::new())
            }
            Message::Done { seq } => {
                self.require_hello()?;
                if self.pieces.waiting() {
                    return Err(VaultError::SyncProtocol("an item was cut short"));
                }
                let received = std::mem::take(&mut self.inbox);
                let peer = session.peer(self.peer)?.clone();
                let report = session.apply_changes_from(&peer, &received)?;
                self.merge_report(report);
                session.record_sync(self.peer, None, Some(seq), Timestamp::now())?;
                self.done_handled = true;
                let mut out = Vec::new();
                if self
                    .last_sent_done
                    .is_some_and(|sent| session.sync_seq() > sent)
                {
                    out = self.send_changes(session, self.last_sent_done)?;
                }
                let missing: Vec<AttachmentId> = session
                    .missing_attachments()?
                    .into_iter()
                    .map(|a| a.id)
                    .filter(|id| self.wanted.insert(*id))
                    .collect();
                if !missing.is_empty() {
                    out.push(Message::WantFiles(missing));
                }
                out.push(Message::Ack { seq });
                Ok(out)
            }
            Message::WantFiles(ids) => {
                self.require_hello()?;
                self.send_files(session, ids)
            }
            Message::FilePart { id, last, bytes } => {
                self.require_hello()?;
                self.receive_file_part(session, id, last, &bytes)?;
                Ok(Vec::new())
            }
            Message::NoFile(id) => {
                self.require_hello()?;
                self.wanted.remove(&id);
                self.receiving.remove(&id);
                Ok(Vec::new())
            }
            Message::Ack { seq } => {
                if self.last_sent_done.is_none_or(|sent| seq > sent) {
                    return Err(VaultError::SyncProtocol("ack for something never sent"));
                }
                self.acked_by_peer = Some(seq);
                session.record_sync(self.peer, Some(seq), None, Timestamp::now())?;
                Ok(Vec::new())
            }
        }
    }

    /// The pieces of every asked-for file this device has (`NoFile` for the others).
    fn send_files(&self, session: &Session, ids: Vec<AttachmentId>) -> Result<Vec<Message>> {
        let peer = session.peer(self.peer)?;
        let allowed = session.attachments_in(|vault| peer.may_use(vault))?;
        let mut out = Vec::new();
        for id in ids {
            let sealed = if allowed.contains(&id) {
                session.sealed_attachment(id)?
            } else {
                None
            };
            match sealed {
                Some(sealed) => out.extend(file_parts(id, &sealed, self.max_message)),
                None => out.push(Message::NoFile(id)),
            }
        }
        Ok(out)
    }

    /// Collects a piece of an asked-for file; stores it once complete.
    fn receive_file_part(
        &mut self,
        session: &mut Session,
        id: AttachmentId,
        last: bool,
        bytes: &[u8],
    ) -> Result<()> {
        if !self.wanted.contains(&id) {
            return Err(VaultError::SyncProtocol("a file nobody asked for"));
        }
        let buffer = self.receiving.entry(id).or_default();
        if u64::try_from(buffer.len() + bytes.len()).unwrap_or(u64::MAX) > MAX_SEALED {
            return Err(VaultError::SyncProtocol("file too large"));
        }
        buffer.extend_from_slice(bytes);
        if last {
            let sealed = self.receiving.remove(&id).unwrap_or_default();
            if session.store_received_attachment(id, &sealed)? {
                self.report.files += 1;
            }
            self.wanted.remove(&id);
        }
        Ok(())
    }

    fn require_hello(&self) -> Result<()> {
        if self.hello_received {
            Ok(())
        } else {
            Err(VaultError::SyncProtocol("message before hello"))
        }
    }

    fn send_changes(&mut self, session: &Session, since: Option<u64>) -> Result<Vec<Message>> {
        let seq = session.sync_seq();
        let peer = session.peer(self.peer)?;
        let mut out = chunk(session.changes_for(peer, since)?, self.max_message)?;
        out.push(Message::Done { seq });
        self.last_sent_done = Some(seq);
        Ok(out)
    }

    fn merge_report(&mut self, report: ApplyReport) {
        for id in report.changed {
            if !self.report.changed.contains(&id) {
                self.report.changed.push(id);
            }
        }
        for id in report.conflicts {
            if !self.report.conflicts.contains(&id) {
                self.report.conflicts.push(id);
            }
        }
        self.report.skipped += report.skipped;
        self.report.password_changed |= report.password_changed;
        self.report.files += report.files;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use password365_crypto::kdf::KdfParams;
    use password365_model::{ItemDetails, ItemKind, ItemOverview};

    use super::*;
    use crate::Store;
    use crate::store::test_support::{credentials, new_session};

    const LIMIT: usize = 4096;

    /// A and a device B that joined it, both paired with each other.
    fn paired() -> (tempfile::TempDir, Session, Session) {
        let (dir, mut a) = new_session();
        a.set_device_name("A").unwrap();
        let vault = a.vault_by_name("Personal").unwrap().id;
        let bundle = a
            .join_bundle(&credentials("master").secret_key, &[vault])
            .unwrap();
        let (mut b, _) = Store::join(
            &dir.path().join("b.qlvault"),
            &bundle,
            "master".into(),
            KdfParams::insecure_for_tests(),
        )
        .unwrap();
        b.set_device_name("B").unwrap();
        a.add_peer(crate::Peer::new(
            b.device_id(),
            "B",
            b.static_key().public(),
            Timestamp::now(),
        ))
        .unwrap();
        (dir, a, b)
    }

    fn add(s: &mut Session, title: &str) -> password365_model::ItemId {
        let v = s.vault_by_name("Personal").unwrap().id;
        s.add_item(
            v,
            ItemKind::Login,
            ItemOverview::new(title).unwrap(),
            ItemDetails::with_password("p".into()),
        )
        .unwrap()
    }

    fn titles(s: &Session) -> Vec<String> {
        s.items().iter().map(|i| i.overview.title.clone()).collect()
    }

    /// Runs a whole conversation over in-memory queues; returns both reports
    /// and the number of messages exchanged.
    fn converse(a: &mut Session, b: &mut Session) -> (ApplyReport, ApplyReport, usize) {
        let (mut sa, hello_a) = PeerSync::start(a, &b.static_key().public(), LIMIT).unwrap();
        let (mut sb, hello_b) = PeerSync::start(b, &a.static_key().public(), LIMIT).unwrap();
        let mut to_b: VecDeque<Message> = VecDeque::from([hello_a]);
        let mut to_a: VecDeque<Message> = VecDeque::from([hello_b]);
        let mut count = 0;
        while !(sa.is_finished() && sb.is_finished()) {
            assert!(count < 1000, "conversation did not end");
            if let Some(m) = to_a.pop_front() {
                count += 1;
                // Every message survives encoding (as on a real channel).
                let m = Message::decode(&m.encode().unwrap()).unwrap();
                to_b.extend(sa.handle(a, m).unwrap());
            }
            if let Some(m) = to_b.pop_front() {
                count += 1;
                to_a.extend(sb.handle(b, m).unwrap());
            }
        }
        assert!(to_a.is_empty() && to_b.is_empty());
        (sa.report().clone(), sb.report().clone(), count)
    }

    /// `b` was given only Personal; `a` also has Work, with item "Payroll".
    fn given_personal_only() -> (
        tempfile::TempDir,
        Session,
        Session,
        password365_model::ItemId,
    ) {
        let (dir, mut a, b) = paired();
        let personal = a.vault_by_name("Personal").unwrap().id;
        let mut peer = a.peer(b.device_id()).unwrap().clone();
        peer.vaults = Some(vec![personal]);
        a.add_peer(peer).unwrap();
        let work = a.create_vault("Work").unwrap();
        let payroll = a
            .add_item(
                work,
                ItemKind::Login,
                ItemOverview::new("Payroll").unwrap(),
                ItemDetails::with_password("p".into()),
            )
            .unwrap();
        (dir, a, b, payroll)
    }

    #[test]
    fn a_device_receives_only_the_vaults_it_was_given() {
        let (_dir, mut a, mut b, payroll) = given_personal_only();
        add(&mut a, "Mail");
        let peer = a.peer(b.device_id()).unwrap().clone();
        let sent = a.changes_for(&peer, None).unwrap();
        assert!(sent.iter().all(|s| s.id() != payroll), "Work never leaves");
        assert_eq!(a.changes_since(None).unwrap().len(), sent.len() + 1);
        converse(&mut a, &mut b);
        assert_eq!(titles(&b), vec!["Mail"]);
    }

    #[test]
    fn a_device_cannot_change_or_move_items_of_vaults_it_was_not_given() {
        let (_dir, mut a, b, payroll) = given_personal_only();
        let personal = a.vault_by_name("Personal").unwrap().id;
        let work = a.vault_by_name("Work").unwrap().id;
        let peer = a.peer(b.device_id()).unwrap().clone();
        let state = |s: &Session, id| {
            s.changes_since(None)
                .unwrap()
                .into_iter()
                .find(|st| st.id() == id)
                .unwrap()
        };
        // An item of Work changed by the device: refused.
        let report = a.apply_changes_from(&peer, &[state(&a, payroll)]).unwrap();
        assert_eq!(report.skipped, 1);
        // Work's item said to be in Personal now (moving it out): refused.
        let mut moved = state(&a, payroll);
        if let ItemState::Live(item) = &mut moved {
            item.fields.vault_id.value = personal;
        }
        assert_eq!(a.apply_changes_from(&peer, &[moved]).unwrap().skipped, 1);
        // A Personal item put in Work by the device: refused.
        let mail = add(&mut a, "Mail");
        let mut into_work = state(&a, mail);
        if let ItemState::Live(item) = &mut into_work {
            item.fields.vault_id.value = work;
        }
        assert_eq!(
            a.apply_changes_from(&peer, &[into_work]).unwrap().skipped,
            1
        );
        // Its own vault: accepted (nothing new to merge here, nothing skipped).
        assert_eq!(
            a.apply_changes_from(&peer, &[state(&a, mail)])
                .unwrap()
                .skipped,
            0
        );
        assert_eq!(a.item(payroll).unwrap().overview.title, "Payroll");
    }

    #[test]
    fn the_vaults_a_device_was_given_survive_reopening() {
        let (dir, a, b, _) = given_personal_only();
        let personal = a.vault_by_name("Personal").unwrap().id;
        a.lock();
        let a = Store::open(&dir.path().join("test.qlvault"))
            .unwrap()
            .unlock(&credentials("master"))
            .unwrap();
        assert_eq!(a.peer(b.device_id()).unwrap().vaults, Some(vec![personal]));
    }

    #[test]
    fn an_item_larger_than_a_message_still_syncs() {
        let (_dir, mut a, mut b) = paired();
        let id = add(&mut a, "Big note");
        let mut item = a.item(id).unwrap();
        item.details.notes = Some("x".repeat(20 * LIMIT).into());
        item.touch(Timestamp::now());
        a.save_item(&item).unwrap();
        add(&mut a, "Small");
        converse(&mut a, &mut b);
        let notes = b.item(id).unwrap().details.notes.unwrap();
        assert_eq!(notes.expose().len(), 20 * LIMIT);
        assert_eq!(titles(&b).len(), 2);
    }

    #[test]
    fn pieces_of_an_item_must_be_complete_and_not_too_large() {
        let mut pieces = crate::wire::ItemPieces::default();
        assert!(pieces.add(false, &[1, 2, 3]).unwrap().is_none());
        assert!(pieces.waiting());
        assert!(pieces.add(true, &[4]).is_err(), "garbage is not an item");
        let mut pieces = crate::wire::ItemPieces::default();
        let big = vec![0; crate::wire::MAX_ITEM_BYTES];
        assert!(pieces.add(false, &big).unwrap().is_none());
        assert!(pieces.add(false, &[0]).is_err());
    }

    #[test]
    fn edits_of_different_custom_fields_on_two_devices_both_survive() {
        let (_dir, mut a, mut b) = paired();
        let id = add(&mut a, "Gym");
        let field = |label: &str, value: &str| password365_model::CustomField {
            label: label.into(),
            value: value.into(),
            concealed: false,
        };
        let mut item = a.item(id).unwrap();
        item.details.custom_fields = vec![field("member", "1"), field("locker", "7")];
        item.touch(Timestamp::now());
        a.save_item(&item).unwrap();
        converse(&mut a, &mut b);

        let mut on_a = a.item(id).unwrap();
        on_a.details.custom_fields[0].value = "2".into();
        on_a.touch(Timestamp::now());
        a.save_item(&on_a).unwrap();
        let mut on_b = b.item(id).unwrap();
        on_b.details.custom_fields[1].value = "9".into();
        on_b.details.custom_fields.push(field("pin", "0000"));
        on_b.touch(Timestamp::now());
        b.save_item(&on_b).unwrap();
        converse(&mut a, &mut b);

        for s in [&a, &b] {
            let values: Vec<String> = s
                .item(id)
                .unwrap()
                .details
                .custom_fields
                .iter()
                .map(|f| format!("{}={}", f.label, f.value.expose()))
                .collect();
            assert_eq!(values, ["member=2", "locker=9", "pin=0000"]);
        }
    }

    #[test]
    fn attached_files_reach_the_other_device() {
        let (_dir, mut a, mut b) = paired();
        let id = add(&mut a, "Passport");
        let big: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        let file = a.attach(id, "scan.bin", "", &big).unwrap();
        let (_, on_b, _) = converse(&mut a, &mut b);
        assert_eq!(on_b.files, 1);
        assert_eq!(
            b.attachment_bytes(id, file.id).unwrap().as_slice(),
            big.as_slice(),
            "split into many parts and rebuilt"
        );
        let (_, again, _) = converse(&mut a, &mut b);
        assert_eq!(again.files, 0, "nothing to send twice");
    }

    #[test]
    fn a_file_neither_device_has_does_not_stall_the_sync() {
        let (_dir, mut a, mut b) = paired();
        let id = add(&mut a, "Scan");
        let file = a.attach(id, "x.txt", "", b"x").unwrap();
        a.db.delete_attachment(file.id.as_bytes()).unwrap();
        let (_, on_b, _) = converse(&mut a, &mut b);
        assert_eq!(on_b.files, 0);
        assert_eq!(
            b.missing_attachments().unwrap().len(),
            1,
            "it will be asked for again next time"
        );
    }

    #[test]
    fn files_nobody_asked_for_are_refused() {
        let (_dir, mut a, mut b) = paired();
        let (mut sb, _) = PeerSync::start(&b, &a.static_key().public(), LIMIT).unwrap();
        let hello = Message::Hello {
            device_id: a.device_id(),
            have_from_you: None,
        };
        sb.handle(&mut b, hello).unwrap();
        let part = Message::FilePart {
            id: password365_model::AttachmentId::new(),
            last: true,
            bytes: vec![1],
        };
        assert!(matches!(
            sb.handle(&mut b, part),
            Err(VaultError::SyncProtocol(_))
        ));
        let _ = &mut a;
    }

    #[test]
    fn a_master_password_change_reaches_the_other_device() {
        let (_d, mut a, mut b) = paired();
        a.change_master_password(&credentials("new"), KdfParams::insecure_for_tests())
            .unwrap();
        let (on_a, on_b, _) = converse(&mut a, &mut b);
        assert!(on_b.password_changed && !on_a.password_changed);
        assert!(b.pending_password_change().is_some());
        let (_, on_b, _) = converse(&mut a, &mut b);
        assert!(!on_b.password_changed, "told only once");
    }

    #[test]
    fn a_device_removed_on_one_device_is_removed_on_the_others() {
        let (_d, mut a, mut b) = paired();
        let stolen = password365_sync::DeviceId::new();
        for s in [&mut a, &mut b] {
            s.add_peer(crate::Peer::new(
                stolen,
                "Old laptop",
                [9; 32],
                Timestamp::now(),
            ))
            .unwrap();
        }
        a.revoke_device(stolen).unwrap();
        converse(&mut a, &mut b);
        assert!(b.is_revoked(stolen));
        assert!(b.peer(stolen).is_err(), "B no longer syncs with it");
    }

    #[test]
    fn a_peer_that_was_removed_elsewhere_cannot_finish_a_sync() {
        let (_d, mut a, b) = paired();
        let (mut sa, _) = PeerSync::start(&a, &b.static_key().public(), LIMIT).unwrap();
        let hello_b = Message::Hello {
            device_id: b.device_id(),
            have_from_you: None,
        };
        sa.handle(&mut a, hello_b).unwrap();
        let err = sa
            .handle(&mut a, Message::Revoked(vec![b.device_id()]))
            .unwrap_err();
        assert!(matches!(err, VaultError::RevokedDevice));
    }

    #[test]
    fn two_devices_exchange_their_changes() {
        let (_d, mut a, mut b) = paired();
        add(&mut a, "From A");
        add(&mut b, "From B");
        let (on_a, on_b, _) = converse(&mut a, &mut b);
        assert_eq!(titles(&a), vec!["From A", "From B"]);
        assert_eq!(titles(&b), vec!["From A", "From B"]);
        assert_eq!((on_a.changed.len(), on_b.changed.len()), (1, 1));
        for (s, other) in [(&a, b.device_id()), (&b, a.device_id())] {
            assert_eq!(
                s.changes_pending_for(other).unwrap(),
                0,
                "nothing left to send"
            );
            assert!(s.peer(other).unwrap().last_sync.is_some());
        }
    }

    #[test]
    fn a_second_sync_sends_only_new_changes() {
        let (_d, mut a, mut b) = paired();
        for i in 0..30 {
            add(&mut a, &format!("item {i}"));
        }
        let (_, first, _) = converse(&mut a, &mut b);
        assert_eq!(first.changed.len(), 30);
        add(&mut a, "new");
        let (_, second, _) = converse(&mut a, &mut b);
        assert_eq!(second.changed.len(), 1);
        let (_, third, messages) = converse(&mut a, &mut b);
        assert!(third.changed.is_empty());
        assert!(
            messages <= 8,
            "an idle sync is a few small messages, got {messages}"
        );
    }

    #[test]
    fn conflicting_passwords_are_reported_and_converge() {
        let (_d, mut a, mut b) = paired();
        let id = add(&mut a, "Mail");
        converse(&mut a, &mut b);
        for (s, pw) in [(&mut a, "pw-a"), (&mut b, "pw-b")] {
            let mut item = s.item(id).unwrap();
            item.set_password(Some(pw.into()), Timestamp::now());
            s.save_item(&item).unwrap();
        }
        let (on_a, on_b, _) = converse(&mut a, &mut b);
        // Each device flags the item so the user can review it wherever they look.
        assert_eq!((on_a.conflicts, on_b.conflicts), (vec![id], vec![id]));
        assert_eq!(a.item(id).unwrap().details, b.item(id).unwrap().details);
        assert_eq!(a.changes_pending_for(b.device_id()).unwrap(), 0);
        assert_eq!(b.changes_pending_for(a.device_id()).unwrap(), 0);
    }

    #[test]
    fn unknown_devices_and_out_of_order_messages_are_refused() {
        let (_d, a, mut b) = paired();
        assert!(matches!(
            PeerSync::start(&a, &[9; 32], LIMIT),
            Err(VaultError::UnknownDevice)
        ));
        let (mut sb, _) = PeerSync::start(&b, &a.static_key().public(), LIMIT).unwrap();
        assert_eq!(sb.peer(), a.device_id());
        assert!(matches!(
            sb.handle(&mut b, Message::Done { seq: 1 }),
            Err(VaultError::SyncProtocol(_))
        ));
        assert!(matches!(
            sb.handle(&mut b, Message::Items(vec![])),
            Err(VaultError::SyncProtocol(_))
        ));
        assert!(matches!(
            sb.handle(&mut b, Message::Ack { seq: 0 }),
            Err(VaultError::SyncProtocol(_))
        ));
        let impostor = Message::Hello {
            device_id: DeviceId::new(),
            have_from_you: None,
        };
        assert!(matches!(
            sb.handle(&mut b, impostor),
            Err(VaultError::SyncProtocol(_))
        ));
        let hello = Message::Hello {
            device_id: a.device_id(),
            have_from_you: None,
        };
        sb.handle(&mut b, hello.clone()).unwrap();
        assert!(
            matches!(sb.handle(&mut b, hello), Err(VaultError::SyncProtocol(_))),
            "second hello"
        );
        let beyond = Message::Ack {
            seq: b.sync_seq() + 1,
        };
        assert!(matches!(
            sb.handle(&mut b, beyond),
            Err(VaultError::SyncProtocol(_))
        ));
        assert!(!sb.is_finished());
    }
}

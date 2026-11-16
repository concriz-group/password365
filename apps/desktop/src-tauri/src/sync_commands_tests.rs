//! Invite → approve on one app state, join on another, over localhost.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::thread;

use super::*;
use crate::state::test_support::state;

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const PW: &str = "correct horse battery";

fn existing() -> (tempfile::TempDir, Arc<AppState>) {
    let (dir, state) = state();
    state
        .with(|s| s.create_account(PW.into(), &PW.into()))
        .unwrap();
    state.with(|s| s.rename_this_device("Laptop")).unwrap();
    (dir, Arc::new(state))
}

#[test]
fn a_new_device_is_invited_approved_and_joins() {
    let (_da, a) = existing();
    let slots_a = Arc::new(PairingSlots::default());
    let invite = invite_on(&a, &slots_a, LOCALHOST).unwrap();
    assert!(invite.qr_svg.starts_with("<?xml") || invite.qr_svg.contains("<svg"));
    let vault = a.with(|s| Ok(s.vaults()?[0].id.clone())).unwrap();

    let (a2, s2) = (Arc::clone(&a), Arc::clone(&slots_a));
    let existing_side = thread::spawn(move || {
        let code = wait_for_device(&s2).unwrap();
        let device = approve_device(&a2, &s2, &[vault]).unwrap();
        (code, device)
    });

    let (_db, b) = state();
    let slots_b = PairingSlots::default();
    let code_on_new = join_connect(&slots_b, &invite.code).unwrap();
    join_wait_approval(&slots_b).unwrap();
    join_wait_approval(&slots_b).unwrap(); // a repeated wait is harmless
    let wrong = join_finish(&b, &slots_b, "wrong".into(), "Phone");
    assert_eq!(wrong.unwrap_err(), AppError::WrongCredentials);
    join_finish(&b, &slots_b, PW.into(), "Phone").unwrap();

    let (code_on_existing, device) = existing_side.join().unwrap();
    assert_eq!(code_on_new, code_on_existing);
    assert_eq!(device.name, "Phone");
    let on_b = b.with(AppService::devices).unwrap();
    assert_eq!(on_b[0].name, "Laptop");
    assert!(b.with(|s| Ok(s.status().unlocked)).unwrap());
    assert!(slots_b.bundle().is_err(), "the bundle is wiped once joined");
}

#[test]
fn no_account_is_created_without_the_connection_to_report_back() {
    let (_db, b) = state();
    let slots = PairingSlots::default();
    slots
        .put_bundle(zeroize::Zeroizing::new(vec![1, 2, 3]))
        .unwrap();
    assert!(join_finish(&b, &slots, PW.into(), "x").is_err());
    assert!(!b.with(|s| Ok(s.status().initialized)).unwrap());
}

#[test]
fn cancelling_an_invite_stops_the_wait() {
    let (_da, a) = existing();
    let slots = Arc::new(PairingSlots::default());
    invite_on(&a, &slots, LOCALHOST).unwrap();
    let waiting = {
        let slots = Arc::clone(&slots);
        thread::spawn(move || wait_for_device(&slots))
    };
    thread::sleep(std::time::Duration::from_millis(150));
    slots.cancel_all().unwrap();
    let err = waiting.join().unwrap().unwrap_err();
    assert!(err.to_string().contains("cancelled"));
}

#[test]
fn steps_out_of_order_explain_themselves() {
    let (_da, a) = existing();
    let slots = PairingSlots::default();
    assert!(wait_for_device(&slots).is_err());
    assert!(approve_device(&a, &slots, &[]).is_err());
    assert!(join_wait_approval(&slots).is_err());
    assert!(join_finish(&a, &slots, PW.into(), "x").is_err());
    assert_eq!(
        join_connect(&slots, "not a code").unwrap_err().code(),
        "validation"
    );
}

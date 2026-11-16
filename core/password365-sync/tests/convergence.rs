//! Property tests: however edits interleave and in whatever order devices
//! exchange state, every device ends with the same item and no password is lost.

use password365_model::{
    CustomField, Item, ItemDetails, ItemKind, ItemOverview, Timestamp, VaultId,
};
use password365_sync::{Clock, DeviceId, ItemState, SyncItem, history};
use proptest::prelude::*;

const DEVICES: usize = 3;
const FIELD_LABELS: [&str; 3] = ["pin", "club", "plan"];

#[derive(Debug, Clone)]
enum Op {
    Title(u8),
    Username(u8),
    Password(u8),
    Favorite(bool),
    /// Sets custom field `pin`/`club`/`plan` (by `.0 % 3`) to a value.
    SetField(u8, u8),
    /// Removes one of those custom fields.
    RemoveField(u8),
    /// Moves the first custom field to the end.
    Reorder,
    Delete,
    /// Device `.0` pulls the state of device `.1`.
    Pull(usize),
}

fn op() -> impl Strategy<Value = (usize, Op)> {
    let kind = prop_oneof![
        any::<u8>().prop_map(Op::Title),
        any::<u8>().prop_map(Op::Username),
        (0u8..6).prop_map(Op::Password),
        any::<bool>().prop_map(Op::Favorite),
        (any::<u8>(), any::<u8>()).prop_map(|(f, v)| Op::SetField(f, v)),
        any::<u8>().prop_map(Op::RemoveField),
        Just(Op::Reorder),
        Just(Op::Delete),
        (0..DEVICES).prop_map(Op::Pull),
    ];
    (0..DEVICES, kind)
}

struct Replica {
    clock: Clock,
    state: ItemState,
    passwords_set: Vec<String>,
}

fn base_item() -> Item {
    let mut item = Item::new(
        VaultId::from_bytes([7; 16]),
        ItemKind::Login,
        ItemOverview::new("Site").expect("valid title"),
        ItemDetails::with_password("initial".into()),
        Timestamp::from_millis(1),
    );
    item.id = password365_model::ItemId::from_bytes([9; 16]);
    item
}

fn replicas() -> Vec<Replica> {
    let origin = DeviceId::from_bytes([0; 16]);
    let state = ItemState::live(SyncItem::capture(&base_item(), Clock::new(origin).tick(1)));
    (0..DEVICES)
        .map(|i| Replica {
            clock: {
                // A device's clock has seen everything it stores.
                let mut clock = Clock::new(DeviceId::from_bytes(
                    [u8::try_from(i + 1).expect("few devices"); 16],
                ));
                clock.observe(state.latest(), 0);
                clock
            },
            state: state.clone(),
            passwords_set: vec!["initial".into()],
        })
        .collect()
}

/// Runs `ops`; physical time advances 1 ms per op but every device's clock is skewed.
fn run(ops: &[(usize, Op)]) -> Vec<Replica> {
    let mut all = replicas();
    for (step, (who, op)) in ops.iter().enumerate() {
        let skew = [0, 1_000, -500][*who];
        let now = i64::try_from(step).expect("short run") + 10 + skew;
        if let Op::Pull(from) = op {
            let theirs = all[*from].state.clone();
            let me = &mut all[*who];
            me.clock.observe(theirs.latest(), now);
            me.state = me.state.merge(&theirs).state;
            let seen = all[*from].passwords_set.clone();
            all[*who].passwords_set.extend(seen);
            continue;
        }
        let me = &mut all[*who];
        let stamp = me.clock.tick(now);
        if matches!(op, Op::Delete) {
            me.state = me.state.clone().deleted(stamp);
            continue;
        }
        // A deleted item cannot be edited locally.
        let ItemState::Live(state) = &mut me.state else {
            continue;
        };
        let mut item = state.to_item(1);
        match op {
            Op::Title(n) => item.overview.title = format!("title-{n}"),
            Op::Username(n) => item.overview.username = Some(format!("user-{n}")),
            Op::Password(n) => {
                let pw = format!("pw-{n}");
                item.set_password(Some(pw.as_str().into()), Timestamp::from_millis(now));
                me.passwords_set.push(pw);
            }
            Op::Favorite(f) => item.overview.favorite = *f,
            Op::SetField(f, v) => {
                let label = FIELD_LABELS[usize::from(*f) % FIELD_LABELS.len()];
                let fields = &mut item.details.custom_fields;
                match fields.iter_mut().find(|c| c.label == label) {
                    Some(c) => c.value = format!("{v}").as_str().into(),
                    None => fields.push(CustomField {
                        label: label.into(),
                        value: format!("{v}").as_str().into(),
                        concealed: false,
                    }),
                }
            }
            Op::RemoveField(f) => {
                let label = FIELD_LABELS[usize::from(*f) % FIELD_LABELS.len()];
                item.details.custom_fields.retain(|c| c.label != label);
            }
            Op::Reorder => {
                if !item.details.custom_fields.is_empty() {
                    let first = item.details.custom_fields.remove(0);
                    item.details.custom_fields.push(first);
                }
            }
            Op::Delete | Op::Pull(_) => unreachable!(),
        }
        state.record(&item, stamp);
    }
    all
}

fn merge_all<'a>(states: impl Iterator<Item = &'a ItemState>) -> ItemState {
    states
        .cloned()
        .reduce(|acc, s| acc.merge(&s).state)
        .expect("some states")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn devices_converge_whatever_the_exchange_order(ops in prop::collection::vec(op(), 0..40)) {
        let all = run(&ops);
        let forward = merge_all(all.iter().map(|r| &r.state));
        let backward = merge_all(all.iter().rev().map(|r| &r.state));
        let middle_first = merge_all([1, 0, 2].iter().map(|&i| &all[i].state));
        prop_assert_eq!(&forward, &backward);
        prop_assert_eq!(&forward, &middle_first);
    }

    #[test]
    fn merge_is_idempotent_commutative_and_associative(ops in prop::collection::vec(op(), 0..40)) {
        let all = run(&ops);
        let (a, b, c) = (&all[0].state, &all[1].state, &all[2].state);
        prop_assert_eq!(&a.merge(a).state, a);
        prop_assert_eq!(a.merge(b).state, b.merge(a).state);
        prop_assert_eq!(a.merge(b).state.merge(c).state, a.merge(&b.merge(c).state).state);
    }

    #[test]
    fn no_password_set_anywhere_is_lost(ops in prop::collection::vec(op(), 0..40)) {
        let all = run(&ops);
        let merged = merge_all(all.iter().map(|r| &r.state));
        // A deleted item is meant to be gone, secrets included.
        let ItemState::Live(merged) = merged else { return Ok(()) };
        let item = merged.to_item(1);
        let distinct: std::collections::BTreeSet<&String> =
            all.iter().flat_map(|r| &r.passwords_set).collect();
        prop_assume!(distinct.len() <= password365_model::password_history::MAX_HISTORY);
        for pw in distinct {
            let secret = pw.as_str().into();
            prop_assert!(
                item.details.password.as_ref() == Some(&secret)
                    || history::contains(&item.details.password_history, &secret),
                "lost {}", pw
            );
        }
    }
}

//! Hybrid Logical Clock: wall-clock milliseconds plus a counter, tie-broken by
//! device. Stamps are totally ordered, unique per device and never go
//! backwards, even when the system clock does.

use serde::{Deserialize, Serialize};

use crate::DeviceId;

/// How far ahead of our own clock a peer's stamp may be before we refuse to
/// adopt its wall time (a peer with a broken clock must not drag ours into the
/// future forever). Its data still merges; only our clock ignores it.
pub const MAX_DRIFT_MS: i64 = 24 * 60 * 60 * 1000;

/// One point in time on one device. Ordering: wall time, then counter, then device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hlc {
    /// Milliseconds since the Unix epoch (logical; never behind any stamp seen).
    pub wall_ms: i64,
    /// Disambiguates events within the same millisecond.
    pub counter: u32,
    /// Device that produced the stamp.
    pub device: DeviceId,
}

impl Hlc {
    /// The smallest possible stamp (for "never written").
    #[must_use]
    pub const fn zero(device: DeviceId) -> Self {
        Self {
            wall_ms: 0,
            counter: 0,
            device,
        }
    }
}

/// Per-device clock state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    last: Hlc,
}

impl Clock {
    /// A fresh clock for `device`.
    #[must_use]
    pub const fn new(device: DeviceId) -> Self {
        Self {
            last: Hlc::zero(device),
        }
    }

    /// The device this clock stamps for.
    #[must_use]
    pub const fn device(&self) -> DeviceId {
        self.last.device
    }

    /// Latest stamp issued or observed.
    #[must_use]
    pub const fn last(&self) -> Hlc {
        self.last
    }

    /// Issues a new stamp for a local change at physical time `now_ms`.
    pub fn tick(&mut self, now_ms: i64) -> Hlc {
        self.advance(self.last.wall_ms.max(now_ms))
    }

    /// Folds in a stamp received from a peer so later local stamps sort after it.
    pub fn observe(&mut self, remote: Hlc, now_ms: i64) {
        if remote.wall_ms > now_ms.saturating_add(MAX_DRIFT_MS) {
            return;
        }
        let wall = self.last.wall_ms.max(remote.wall_ms).max(now_ms);
        let counter = match (wall == self.last.wall_ms, wall == remote.wall_ms) {
            (true, true) => self.last.counter.max(remote.counter),
            (true, false) => self.last.counter,
            (false, true) => remote.counter,
            (false, false) => 0,
        };
        self.last = Hlc {
            wall_ms: wall,
            counter,
            device: self.last.device,
        };
    }

    fn advance(&mut self, wall_ms: i64) -> Hlc {
        let counter = if wall_ms == self.last.wall_ms {
            self.last.counter + 1
        } else {
            0
        };
        self.last = Hlc {
            wall_ms,
            counter,
            device: self.last.device,
        };
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(n: u8) -> DeviceId {
        DeviceId::from_bytes([n; 16])
    }

    #[test]
    fn ticks_follow_the_wall_clock() {
        let mut c = Clock::new(dev(1));
        let a = c.tick(100);
        let b = c.tick(200);
        assert_eq!((a.wall_ms, a.counter), (100, 0));
        assert_eq!((b.wall_ms, b.counter), (200, 0));
        assert_eq!(c.device(), dev(1));
    }

    #[test]
    fn same_millisecond_bumps_the_counter() {
        let mut c = Clock::new(dev(1));
        let a = c.tick(100);
        let b = c.tick(100);
        assert!(b > a);
        assert_eq!(b.counter, 1);
    }

    #[test]
    fn never_goes_backwards_when_the_system_clock_does() {
        let mut c = Clock::new(dev(1));
        let a = c.tick(500);
        let b = c.tick(10);
        assert!(b > a);
        assert_eq!(b.wall_ms, 500);
    }

    #[test]
    fn observing_a_newer_peer_moves_us_past_it() {
        let mut c = Clock::new(dev(1));
        c.tick(100);
        let remote = Hlc {
            wall_ms: 900,
            counter: 4,
            device: dev(2),
        };
        c.observe(remote, 150);
        assert_eq!(c.last().wall_ms, 900);
        let next = c.tick(150);
        assert!(next > remote);
        assert_eq!(next.device, dev(1));
    }

    #[test]
    fn observing_combines_counters_on_equal_wall_time() {
        let mut c = Clock::new(dev(1));
        c.tick(100);
        c.tick(100);
        c.observe(
            Hlc {
                wall_ms: 100,
                counter: 7,
                device: dev(2),
            },
            50,
        );
        assert_eq!(c.last().counter, 7);
        c.observe(
            Hlc {
                wall_ms: 100,
                counter: 3,
                device: dev(2),
            },
            50,
        );
        assert_eq!(c.last().counter, 7);
    }

    #[test]
    fn observing_an_older_peer_keeps_our_time() {
        let mut c = Clock::new(dev(1));
        c.tick(500);
        c.observe(
            Hlc {
                wall_ms: 100,
                counter: 9,
                device: dev(2),
            },
            400,
        );
        assert_eq!((c.last().wall_ms, c.last().counter), (500, 0));
    }

    #[test]
    fn observing_resets_the_counter_when_our_physical_clock_is_ahead() {
        let mut c = Clock::new(dev(1));
        c.tick(100);
        c.observe(
            Hlc {
                wall_ms: 200,
                counter: 9,
                device: dev(2),
            },
            300,
        );
        assert_eq!((c.last().wall_ms, c.last().counter), (300, 0));
    }

    #[test]
    fn a_peer_far_in_the_future_does_not_drag_our_clock() {
        let mut c = Clock::new(dev(1));
        c.tick(1_000);
        c.observe(
            Hlc {
                wall_ms: 1_000 + MAX_DRIFT_MS + 1_001,
                counter: 0,
                device: dev(2),
            },
            1_000,
        );
        assert_eq!(c.last().wall_ms, 1_000);
    }

    #[test]
    fn device_breaks_ties() {
        let a = Hlc {
            wall_ms: 1,
            counter: 0,
            device: dev(1),
        };
        let b = Hlc {
            wall_ms: 1,
            counter: 0,
            device: dev(2),
        };
        assert!(a < b);
        assert!(Hlc::zero(dev(9)) < a);
    }
}

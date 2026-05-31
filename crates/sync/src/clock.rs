use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlcTimestamp {
    pub wall_ms: i64,
    pub counter: u32,
}

impl HlcTimestamp {
    pub const ZERO: HlcTimestamp = HlcTimestamp {
        wall_ms: 0,
        counter: 0,
    };
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.wall_ms
            .cmp(&other.wall_ms)
            .then(self.counter.cmp(&other.counter))
    }
}

pub trait WallClock {
    fn now_ms(&self) -> i64;
}

pub struct StdClock;

impl WallClock for StdClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

pub struct HybridClock<W: WallClock> {
    wall_clock: W,
    node_id: NodeId,
    last_ts: HlcTimestamp,
}

impl<W: WallClock> HybridClock<W> {
    pub fn new(node_id: NodeId, wall_clock: W) -> Self {
        HybridClock {
            wall_clock,
            node_id,
            last_ts: HlcTimestamp::ZERO,
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn now(&mut self) -> HlcTimestamp {
        let physical = self.wall_clock.now_ms();
        let ts = if physical > self.last_ts.wall_ms {
            HlcTimestamp {
                wall_ms: physical,
                counter: 0,
            }
        } else {
            HlcTimestamp {
                wall_ms: self.last_ts.wall_ms,
                counter: self.last_ts.counter.saturating_add(1),
            }
        };
        self.last_ts = ts;
        ts
    }

    pub fn recv(&mut self, remote: HlcTimestamp) -> HlcTimestamp {
        let physical = self.wall_clock.now_ms();
        let max_wall = physical.max(self.last_ts.wall_ms).max(remote.wall_ms);

        let counter = if max_wall == self.last_ts.wall_ms && max_wall == remote.wall_ms {
            self.last_ts.counter.max(remote.counter).saturating_add(1)
        } else if max_wall == self.last_ts.wall_ms {
            self.last_ts.counter.saturating_add(1)
        } else if max_wall == remote.wall_ms {
            remote.counter.saturating_add(1)
        } else {
            0
        };

        let ts = HlcTimestamp {
            wall_ms: max_wall,
            counter,
        };
        self.last_ts = ts;
        ts
    }

    pub fn last(&self) -> HlcTimestamp {
        self.last_ts
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct FixedClock(i64);
    impl WallClock for FixedClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    #[test]
    fn now_monotonic() {
        let mut clock = HybridClock::new(NodeId(Uuid::now_v7()), StdClock);
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2 > t1);
    }

    #[test]
    fn now_with_stale_wall_clock() {
        let mut clock = HybridClock::new(NodeId(Uuid::now_v7()), FixedClock(1000));
        let t1 = clock.now();
        assert_eq!(t1.wall_ms, 1000);
        assert_eq!(t1.counter, 0);
        let t2 = clock.now();
        assert_eq!(t2.wall_ms, 1000);
        assert_eq!(t2.counter, 1);
        assert!(t2 > t1);
    }

    #[test]
    fn recv_remote_ahead() {
        let mut clock = HybridClock::new(NodeId(Uuid::now_v7()), FixedClock(1000));
        let _ = clock.now();
        let remote = HlcTimestamp {
            wall_ms: 2000,
            counter: 5,
        };
        let merged = clock.recv(remote);
        assert!(merged > remote);
        assert_eq!(merged.wall_ms, 2000);
        assert_eq!(merged.counter, 6);
    }

    #[test]
    fn recv_remote_behind() {
        let mut clock = HybridClock::new(NodeId(Uuid::now_v7()), FixedClock(2000));
        let _ = clock.now();
        let remote = HlcTimestamp {
            wall_ms: 1000,
            counter: 0,
        };
        let merged = clock.recv(remote);
        assert!(merged.wall_ms >= 2000);
        assert!(merged > remote);
    }

    #[test]
    fn recv_same_wall_ms() {
        let mut clock = HybridClock::new(NodeId(Uuid::now_v7()), FixedClock(1000));
        let _ = clock.now();
        let remote = HlcTimestamp {
            wall_ms: 1000,
            counter: 3,
        };
        let merged = clock.recv(remote);
        assert_eq!(merged.wall_ms, 1000);
        assert_eq!(merged.counter, 4);
    }
}

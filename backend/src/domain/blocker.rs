//! When a blocker earns a nudge, and which one.

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

#[derive(Debug, Clone, Copy)]
pub struct NudgeState {
    pub due_date: Option<NaiveDate>,
    pub resolved: bool,
    pub nudge_enabled: bool,
    pub last_nudged_at: Option<DateTime<Utc>>,
    pub nudge_count: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct NudgePolicy {
    pub interval_days: i32,
    /// After this many nudges have gone out, the escalated template is used.
    pub escalate_after: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeDecision {
    NotDue,
    Nudge { escalated: bool, days_overdue: i64 },
}

pub fn decide(
    state: NudgeState,
    policy: NudgePolicy,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> NudgeDecision {
    if state.resolved || !state.nudge_enabled {
        return NudgeDecision::NotDue;
    }
    let Some(due) = state.due_date else {
        return NudgeDecision::NotDue;
    };
    if today <= due {
        return NudgeDecision::NotDue;
    }
    if let Some(last) = state.last_nudged_at {
        if now - last < TimeDelta::days(i64::from(policy.interval_days)) {
            return NudgeDecision::NotDue;
        }
    }
    NudgeDecision::Nudge {
        escalated: state.nudge_count >= policy.escalate_after,
        days_overdue: (today - due).num_days(),
    }
}

/// One nudge number per blocker can only ever be queued once, even if the scheduler
/// runs twice concurrently.
pub fn nudge_idempotency_key(blocker_id: i64, nudge_number: i32) -> String {
    format!("nudge:{blocker_id}:{nudge_number}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: NudgePolicy = NudgePolicy {
        interval_days: 3,
        escalate_after: 2,
    };

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn open(due: &str) -> NudgeState {
        NudgeState {
            due_date: Some(d(due)),
            resolved: false,
            nudge_enabled: true,
            last_nudged_at: None,
            nudge_count: 0,
        }
    }

    #[test]
    fn not_before_due_date_passes() {
        assert_eq!(
            decide(
                open("2026-09-10"),
                POLICY,
                d("2026-09-10"),
                t("2026-09-10T10:00:00Z")
            ),
            NudgeDecision::NotDue
        );
        assert_eq!(
            decide(
                open("2026-09-10"),
                POLICY,
                d("2026-09-11"),
                t("2026-09-11T10:00:00Z")
            ),
            NudgeDecision::Nudge {
                escalated: false,
                days_overdue: 1
            }
        );
    }

    #[test]
    fn resolved_disabled_or_undated_never_nudge() {
        let now = t("2026-12-01T10:00:00Z");
        assert_eq!(
            decide(
                NudgeState {
                    resolved: true,
                    ..open("2026-09-01")
                },
                POLICY,
                d("2026-12-01"),
                now
            ),
            NudgeDecision::NotDue
        );
        assert_eq!(
            decide(
                NudgeState {
                    nudge_enabled: false,
                    ..open("2026-09-01")
                },
                POLICY,
                d("2026-12-01"),
                now
            ),
            NudgeDecision::NotDue
        );
        assert_eq!(
            decide(
                NudgeState {
                    due_date: None,
                    ..open("2026-09-01")
                },
                POLICY,
                d("2026-12-01"),
                now
            ),
            NudgeDecision::NotDue
        );
    }

    #[test]
    fn respects_interval_then_escalates() {
        let nudged = NudgeState {
            last_nudged_at: Some(t("2026-09-11T08:00:00Z")),
            nudge_count: 1,
            ..open("2026-09-10")
        };
        assert_eq!(
            decide(nudged, POLICY, d("2026-09-13"), t("2026-09-13T08:00:00Z")),
            NudgeDecision::NotDue
        );
        assert_eq!(
            decide(nudged, POLICY, d("2026-09-14"), t("2026-09-14T08:00:00Z")),
            NudgeDecision::Nudge {
                escalated: false,
                days_overdue: 4
            }
        );
        let twice = NudgeState {
            nudge_count: 2,
            ..nudged
        };
        assert!(matches!(
            decide(twice, POLICY, d("2026-09-20"), t("2026-09-20T08:00:00Z")),
            NudgeDecision::Nudge {
                escalated: true,
                ..
            }
        ));
    }
}

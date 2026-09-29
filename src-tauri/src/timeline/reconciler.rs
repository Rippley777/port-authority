use super::{correlation::same_service, models::*};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Reconciler {
    pub previous: Vec<Observation>,
    pub reclaim_window_seconds: u64,
    released: BTreeMap<String, (u64, Observation, String)>,
}
impl Default for Reconciler {
    fn default() -> Self {
        Self {
            previous: vec![],
            released: BTreeMap::new(),
            reclaim_window_seconds: 600,
        }
    }
}
pub fn same_owner(a: &Observation, b: &Observation) -> bool {
    a.endpoint() == b.endpoint()
        && match (a.process.identity(), b.process.identity()) {
            (Some(a), Some(b)) => a == b,
            // Unknown identities cannot prove a restart. Preserve a stable observation until
            // ownership can be inspected; never correlate a PID alone across a release.
            _ => a.process.pid == b.process.pid,
        }
}
impl Reconciler {
    pub fn reconcile(
        &mut self,
        current: Vec<Observation>,
        at: u64,
        session: &str,
        gap: bool,
    ) -> Vec<Event> {
        if gap {
            self.released.clear();
        }
        self.released
            .retain(|_, (time, _, _)| at.saturating_sub(*time) <= self.reclaim_window_seconds);
        let mut events = vec![];
        let removed: Vec<_> = self
            .previous
            .iter()
            .filter(|a| !current.iter().any(|b| same_owner(a, b)))
            .cloned()
            .collect();
        let added: Vec<_> = current
            .iter()
            .filter(|b| !self.previous.iter().any(|a| same_owner(a, b)))
            .cloned()
            .collect();
        let mut paired = std::collections::HashSet::new();
        for b in &added {
            let candidates: Vec<_> = removed
                .iter()
                .filter(|a| a.endpoint() == b.endpoint())
                .collect();
            let unambiguous = candidates.len() == 1
                && added
                    .iter()
                    .filter(|a| a.endpoint() == b.endpoint())
                    .count()
                    == 1;
            let mut previous = None;
            let mut correlation = None;
            let kind = if unambiguous {
                let a = candidates[0];
                paired.insert(a.process.id.clone());
                previous = Some(a.clone());
                if !gap && same_service(a, b) {
                    EventType::ProcessRestarted
                } else {
                    EventType::OwnerChanged
                }
            } else if !gap {
                if let Some((released_at, a, id)) = self.released.remove(&b.endpoint()) {
                    let kind = if at.saturating_sub(released_at) <= 60 && same_service(&a, b) {
                        EventType::ProcessRestarted
                    } else {
                        EventType::PortReclaimed
                    };
                    previous = Some(a);
                    correlation = Some(id);
                    kind
                } else {
                    EventType::PortClaimed
                }
            } else {
                EventType::Observed
            };
            events.push(event(b, previous, kind, at, session, gap, correlation));
        }
        for a in removed {
            if paired.contains(&a.process.id) {
                continue;
            }
            let mut e = event(
                &a,
                Some(a.clone()),
                EventType::PortReleased,
                at,
                session,
                gap,
                None,
            );
            e.endpoint_available = !current.iter().any(|b| a.endpoint() == b.endpoint());
            if !gap && e.endpoint_available {
                self.released.insert(a.endpoint(), (at, a, e.id.clone()));
            }
            events.push(e);
        }
        // A fresh baseline bounds ownership sessions on either side of an observation gap.
        if gap {
            for b in &current {
                if !added.iter().any(|a| same_owner(a, b)) {
                    events.push(event(b, None, EventType::Observed, at, session, true, None));
                }
            }
        }
        self.previous = current;
        events
    }
}
fn event(
    p: &Observation,
    previous: Option<Observation>,
    kind: EventType,
    at: u64,
    session: &str,
    uncertain: bool,
    correlation_id: Option<String>,
) -> Event {
    Event {
        id: uuid::Uuid::new_v4().to_string(),
        sequence: 0,
        timestamp: at,
        session_id: session.into(),
        port: p.process.port,
        protocol: p.process.protocol.clone(),
        address: p.process.address.clone(),
        event_type: kind,
        process: if kind == EventType::PortReleased {
            None
        } else {
            Some(p.clone())
        },
        previous_process: previous,
        uncertain,
        endpoint_available: false,
        correlation_id,
    }
}

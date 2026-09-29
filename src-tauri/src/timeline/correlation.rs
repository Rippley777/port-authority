use super::models::*;

pub fn same_service(a: &Observation, b: &Observation) -> bool {
    let (a, b) = (&a.process, &b.process);
    if a.identity().is_none() || b.identity().is_none() || a.identity() == b.identity() {
        return false;
    }
    let same_location = match (&a.project, &b.project) {
        (Some(a), Some(b))
            if a.confidence != crate::projects::models::Confidence::Low
                && b.confidence != crate::projects::models::Confidence::Low =>
        {
            a.root_path == b.root_path
        }
        _ => a.cwd.is_some() && a.cwd == b.cwd,
    };
    same_location
        && a.executable.is_some()
        && a.executable == b.executable
        && !a.command.is_empty()
        && a.command == b.command
}

pub fn recurring(events: &[Event], config: &Config, now: u64) -> Vec<Recurrence> {
    let mut groups = std::collections::BTreeMap::<String, Vec<&Event>>::new();
    for e in events {
        if !e.uncertain
            && e.timestamp >= now.saturating_sub(config.reclaim_window_seconds)
            && matches!(
                e.event_type,
                EventType::ProcessRestarted | EventType::PortReclaimed
            )
        {
            groups
                .entry(format!(
                    "{}:{}:{}:{}",
                    e.session_id, e.port, e.protocol, e.address
                ))
                .or_default()
                .push(e);
        }
    }
    groups
        .into_values()
        .filter_map(|mut es| {
            es.sort_by_key(|e| e.sequence);
            let latest = *es.last()?;
            let process = latest.process.as_ref()?;
            // Only explain a coherent service, never combine unrelated projects on a port.
            es.retain(|e| {
                e.process.as_ref().is_some_and(|p| {
                    p.process.identity() == process.process.identity() || same_service(p, process)
                })
            });
            if es.len() < config.reclaim_threshold {
                return None;
            }
            let persistent_ancestor = process
                .ancestors
                .iter()
                .find(|ancestor| {
                    es.iter().all(|e| {
                        [e.process.as_ref(), e.previous_process.as_ref()]
                            .into_iter()
                            .all(|p| {
                                p.is_some_and(|p| {
                                    p.ancestors.iter().any(|a| a.identity == ancestor.identity)
                                })
                            })
                    })
                })
                .cloned();
            let mut previous_pids: Vec<_> = es
                .iter()
                .flat_map(|e| [&e.previous_process, &e.process])
                .filter_map(|p| p.as_ref()?.process.pid)
                .collect();
            previous_pids.sort_unstable();
            previous_pids.dedup();
            Some(Recurrence {
                session_id: latest.session_id.clone(),
                port: latest.port,
                protocol: latest.protocol.clone(),
                address: latest.address.clone(),
                count: es.len(),
                first_at: es.first()?.timestamp,
                latest_at: latest.timestamp,
                persistent_ancestor,
                process: process.clone(),
                previous_pids,
            })
        })
        .collect()
}

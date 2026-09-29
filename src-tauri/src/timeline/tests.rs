use super::{correlation, models::*, reconciler::Reconciler, repository::Repository};
use crate::ports::models::PortEntry;
fn owner(pid: u32, start: u64, cwd: &str) -> Observation {
    Observation {
        process: PortEntry {
            id: format!("TCP:127.0.0.1:5173:{pid}"),
            port: 5173,
            protocol: "TCP".into(),
            address: "127.0.0.1".into(),
            pid: Some(pid),
            started_at: Some(start),
            process: "node".into(),
            command: vec!["node".into(), "vite".into()],
            executable: Some("/usr/bin/node".into()),
            cwd: Some(cwd.into()),
            parent_pid: Some(90),
            user: Some("developer".into()),
            project: None,
            service_name: Some("Vite".into()),
            memory: None,
            cpu: None,
            system: false,
            protected: false,
            restartable: false,
            restart_reason: "".into(),
            permission_limited: false,
        },
        ancestors: vec![Ancestor {
            identity: ProcessIdentity {
                pid: 90,
                started_at: 1,
            },
            name: "npm".into(),
            command: vec!["npm".into(), "run".into(), "dev".into()],
            cwd: Some(cwd.into()),
            parent_pid: Some(80),
        }],
    }
}
fn session() -> MonitoringSession {
    MonitoringSession {
        id: "s".into(),
        started_at: 10,
        last_observed: 100,
        ended_at: None,
        reason: "resumed".into(),
    }
}
fn repo() -> Repository {
    Repository::open(std::path::Path::new(":memory:")).unwrap()
}
#[test]
fn claimed_released_restarted_and_idle() {
    let mut r = Reconciler::default();
    let a = owner(100, 10, "/shipwreck");
    assert_eq!(
        r.reconcile(vec![a.clone()], 10, "s", false)[0].event_type,
        EventType::PortClaimed
    );
    assert!(r.reconcile(vec![a], 11, "s", false).is_empty());
    let released = r.reconcile(vec![], 12, "s", false);
    assert_eq!(released[0].event_type, EventType::PortReleased);
    assert_eq!(
        released[0]
            .previous_process
            .as_ref()
            .unwrap()
            .process
            .cwd
            .as_deref(),
        Some("/shipwreck")
    );
    let restart = r.reconcile(vec![owner(200, 13, "/shipwreck")], 13, "s", false);
    assert_eq!(restart.len(), 1);
    assert_eq!(restart[0].event_type, EventType::ProcessRestarted);
    assert_eq!(
        restart[0].correlation_id.as_deref(),
        Some(released[0].id.as_str())
    );
}
#[test]
fn owner_changes_and_direct_restart() {
    let mut r = Reconciler::default();
    r.reconcile(vec![owner(100, 10, "/shipwreck")], 10, "s", false);
    assert_eq!(
        r.reconcile(vec![owner(200, 11, "/plants")], 11, "s", false)[0].event_type,
        EventType::OwnerChanged
    );
    assert_eq!(
        r.reconcile(vec![owner(300, 12, "/plants")], 12, "s", false)[0].event_type,
        EventType::ProcessRestarted
    );
}
#[test]
fn pid_reuse_is_a_new_identity() {
    let mut r = Reconciler::default();
    r.reconcile(vec![owner(100, 10, "/shipwreck")], 10, "s", false);
    let es = r.reconcile(vec![owner(100, 20, "/other")], 20, "s", false);
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].event_type, EventType::OwnerChanged);
    assert_ne!(
        es[0].process.as_ref().unwrap().process.identity(),
        es[0].previous_process.as_ref().unwrap().process.identity()
    );
}
#[test]
fn unknown_identity_does_not_support_restart() {
    let mut r = Reconciler::default();
    let mut a = owner(100, 10, "/a");
    a.process.started_at = None;
    r.reconcile(vec![a], 10, "s", false);
    r.reconcile(vec![], 11, "s", false);
    assert_eq!(
        r.reconcile(vec![owner(200, 12, "/a")], 12, "s", false)[0].event_type,
        EventType::PortReclaimed
    );
}
#[test]
fn separate_protocols_addresses_and_shared_bindings() {
    let mut r = Reconciler::default();
    let a = owner(100, 10, "/a");
    let mut udp = a.clone();
    udp.process.protocol = "UDP".into();
    udp.process.id = "udp".into();
    let mut ipv6 = a.clone();
    ipv6.process.address = "::1".into();
    ipv6.process.id = "v6".into();
    assert_eq!(
        r.reconcile(vec![a.clone(), udp, ipv6.clone()], 10, "s", false)
            .len(),
        3
    );
    let es = r.reconcile(vec![a.clone(), ipv6], 11, "s", false);
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].protocol, "UDP");
    let b = owner(200, 12, "/b");
    let es = r.reconcile(vec![a.clone(), b.clone()], 12, "s", false);
    assert!(es.iter().any(|e| e.event_type == EventType::PortClaimed));
    let es = r.reconcile(vec![b], 13, "s", false);
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].event_type, EventType::PortReleased);
}
#[test]
fn monitoring_gap_is_uncertain_and_breaks_restart_correlation() {
    let mut r = Reconciler::default();
    r.reconcile(vec![owner(100, 10, "/a")], 10, "s", false);
    let es = r.reconcile(vec![owner(200, 20, "/a")], 500, "next", true);
    assert_eq!(es[0].event_type, EventType::OwnerChanged);
    assert!(es[0].uncertain);
    assert_eq!(es[0].timestamp, 500);
    r.reconcile(vec![], 501, "next", false);
    let es = r.reconcile(vec![owner(300, 502, "/a")], 900, "third", true);
    assert_eq!(es[0].event_type, EventType::Observed);
}
#[test]
fn unchanged_owner_gets_baseline_after_gap() {
    let mut r = Reconciler::default();
    let a = owner(100, 10, "/a");
    r.reconcile(vec![a.clone()], 10, "s", false);
    let es = r.reconcile(vec![a], 1000, "new", true);
    assert_eq!(es.len(), 1);
    assert_eq!(es[0].event_type, EventType::Observed);
    assert!(es[0].uncertain);
}
#[test]
fn long_reclaim_is_not_called_a_restart() {
    let mut r = Reconciler::default();
    r.reconcile(vec![owner(100, 10, "/a")], 10, "s", false);
    r.reconcile(vec![], 11, "s", false);
    assert_eq!(
        r.reconcile(vec![owner(200, 100, "/a")], 100, "s", false)[0].event_type,
        EventType::PortReclaimed
    );
}
#[test]
fn recurrence_requires_persistent_identity_in_every_ancestry() {
    let mut r = Reconciler::default();
    let mut es = r.reconcile(vec![owner(100, 10, "/a")], 10, "s", false);
    for i in 1..=3 {
        es.extend(r.reconcile(
            vec![owner(100 + i, 10 + i as u64, "/a")],
            10 + i as u64,
            "s",
            false,
        ));
    }
    for (i, e) in es.iter_mut().enumerate() {
        e.sequence = i as i64;
    }
    let config = Config::default();
    let recurring = correlation::recurring(&es, &config, 20);
    assert_eq!(recurring.len(), 1);
    assert_eq!(recurring[0].count, 3);
    assert_eq!(
        recurring[0]
            .persistent_ancestor
            .as_ref()
            .unwrap()
            .identity
            .pid,
        90
    );
    es[1].process.as_mut().unwrap().ancestors[0]
        .identity
        .started_at = 2;
    assert!(correlation::recurring(&es, &config, 20)[0]
        .persistent_ancestor
        .is_none());
    assert!(correlation::recurring(&es, &config, 1000).is_empty());
}
#[test]
fn database_queries_snapshots_retention_clear_and_keyset_pagination() {
    let mut db = repo();
    let mut r = Reconciler::default();
    let s = session();
    for i in 0..5 {
        let es = r.reconcile(
            vec![owner(100 + i, 10 + i as u64, "/shipwreck")],
            10 + i as u64,
            "s",
            false,
        );
        db.record(&es, &r.previous, &s, None).unwrap();
    }
    let es = db
        .query(&Query {
            port: Some(5173),
            search: Some("shipwreck".into()),
            limit: Some(2),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(es.len(), 2);
    assert!(es[0].sequence > es[1].sequence);
    let older = db
        .query(&Query {
            before: Some(es[1].sequence),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(older.len(), 3);
    let identity = db
        .query(&Query {
            search: Some("PID 104".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(identity.len(), 1);
    let snapshots: Vec<Observation> = db.state("owners").unwrap();
    assert_eq!(snapshots[0].process.pid, Some(104));
    assert!(db
        .query(&Query {
            port: Some(3000),
            ..Default::default()
        })
        .unwrap()
        .is_empty());
    db.prune(1, 86400 + 13).unwrap();
    assert_eq!(db.query(&Query::default()).unwrap().len(), 2);
    db.clear().unwrap();
    assert!(db.query(&Query::default()).unwrap().is_empty());
    assert!(db.sessions().unwrap().is_empty());
}
#[test]
fn history_survives_reopening_and_crashed_session_is_conservative() {
    let path = std::env::temp_dir().join(format!("pa-timeline-{}.sqlite", uuid::Uuid::new_v4()));
    {
        let mut db = Repository::open(&path).unwrap();
        let mut r = Reconciler::default();
        let es = r.reconcile(vec![owner(100, 10, "/a")], 10, "s", true);
        db.record(&es, &r.previous, &session(), None).unwrap();
    }
    {
        let t = super::Timeline::new(&path);
        let page = t.query(Query::default()).unwrap();
        assert_eq!(page.sessions[0].ended_at, Some(100));
        assert!(page.sessions[0]
            .reason
            .contains("exact stop time unavailable"));
    }
    let _ = std::fs::remove_file(path);
}
#[test]
fn idle_service_scans_do_not_write_and_pause_resume_is_a_new_session() {
    let mut t = super::Timeline::new(std::path::Path::new(":memory:"));
    let projects = crate::projects::cache::ProjectEngine::new(None);
    let p = owner(999991, 10, "/missing").process;
    t.observe(std::slice::from_ref(&p), &projects);
    let before = t.repo.as_ref().unwrap().db.total_changes();
    for _ in 0..10 {
        t.observe(std::slice::from_ref(&p), &projects);
    }
    assert_eq!(t.repo.as_ref().unwrap().db.total_changes(), before);
    t.pause("test pause");
    t.observe(&[p], &projects);
    assert_eq!(t.query(Query::default()).unwrap().sessions.len(), 2);
}
#[test]
fn unavailable_database_does_not_panic_or_advance_history() {
    let mut t = super::Timeline::new(std::path::Path::new("/dev/null/impossible.sqlite"));
    t.observe(&[], &crate::projects::cache::ProjectEngine::new(None));
    assert!(t.error.is_some());
    assert!(t.query(Query::default()).is_err());
}
#[test]
fn database_failure_rolls_back_reconciliation() {
    let mut t = super::Timeline::new(std::path::Path::new(":memory:"));
    let projects = crate::projects::cache::ProjectEngine::new(None);
    t.observe(&[], &projects);
    t.repo
        .as_ref()
        .unwrap()
        .db
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    t.observe(&[owner(999991, 10, "/missing").process], &projects);
    assert!(t.error.is_some());
    assert!(t.reconciler.previous.is_empty());
    t.repo
        .as_ref()
        .unwrap()
        .db
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    t.observe(&[owner(999991, 10, "/missing").process], &projects);
    assert_eq!(t.reconciler.previous.len(), 1);
    assert!(t.error.is_none());
}

#[test]
fn tree_control_rejects_missing_changed_or_protected_identities() {
    let missing = ProcessIdentity {
        pid: std::process::id(),
        started_at: 0,
    };
    assert!(super::ancestry::inspect_tree(missing).is_err());
    let mut system = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let me = ProcessIdentity {
        pid: pid.as_u32(),
        started_at: system.process(pid).unwrap().start_time(),
    };
    assert!(super::ancestry::inspect_tree(me)
        .unwrap_err()
        .contains("protected"));
    assert!(super::ancestry::stop_tree(me, vec![me]).is_err());
}
#[test]
fn simulated_sleep_and_clock_rollback_create_observation_boundaries() {
    let mut t = super::Timeline::new(std::path::Path::new(":memory:"));
    let projects = crate::projects::cache::ProjectEngine::new(None);
    let p = owner(999991, 10, "/missing").process;
    t.observe(std::slice::from_ref(&p), &projects);
    let at = super::now();
    t.last_scan = Some((at.saturating_sub(3600), std::time::Instant::now()));
    t.observe(std::slice::from_ref(&p), &projects);
    assert_eq!(t.query(Query::default()).unwrap().sessions.len(), 2);
    t.last_scan = Some((at + 3600, std::time::Instant::now()));
    t.observe(&[p], &projects);
    let page = t.query(Query::default()).unwrap();
    assert_eq!(page.sessions.len(), 3);
    assert!(page.events.iter().all(|e| e.uncertain));
}
#[test]
fn process_metadata_and_project_are_immutable_after_exit() {
    let mut r = Reconciler::default();
    let mut a = owner(100, 10, "/shipwreck");
    a.process.project = Some(crate::projects::models::ProjectIdentity {
        name: "Shipwreck".into(),
        root_path: "/shipwreck".into(),
        display_path: "~/shipwreck".into(),
        project_type: None,
        frameworks: vec!["Vite".into()],
        repository: None,
        git_branch: None,
        git_dirty: None,
        confidence: crate::projects::models::Confidence::High,
        evidence: "manifest".into(),
        manifests: vec![],
    });
    let es = r.reconcile(vec![a], 10, "s", false);
    let mut db = repo();
    db.record(&es, &r.previous, &session(), None).unwrap();
    r.reconcile(vec![owner(100, 20, "/plants")], 20, "s", false);
    let stored = db.query(&Query::default()).unwrap();
    let p = &stored[0].process.as_ref().unwrap().process;
    assert_eq!(p.project.as_ref().unwrap().name, "Shipwreck");
    assert_eq!(p.started_at, Some(10));
    assert_eq!(p.command, vec!["node", "vite"]);
}
#[test]
fn disabled_history_does_not_write_and_retention_forever_keeps_events() {
    let mut t = super::Timeline::new(std::path::Path::new(":memory:"));
    let projects = crate::projects::cache::ProjectEngine::new(None);
    t.configure(Config {
        enabled: false,
        ..Default::default()
    })
    .unwrap();
    let before = t.repo.as_ref().unwrap().db.total_changes();
    t.observe(&[owner(999991, 10, "/missing").process], &projects);
    assert_eq!(t.repo.as_ref().unwrap().db.total_changes(), before);
    let mut db = repo();
    let mut r = Reconciler::default();
    let es = r.reconcile(vec![owner(100, 10, "/a")], 10, "s", false);
    db.record(&es, &r.previous, &session(), None).unwrap();
    db.prune(0, 999999999).unwrap();
    assert_eq!(db.query(&Query::default()).unwrap().len(), 1);
}

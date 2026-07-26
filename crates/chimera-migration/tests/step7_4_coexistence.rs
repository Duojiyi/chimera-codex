// Step 7.4 RED — read-only detection of other Codex/Chimera managers, and
// the confirmation-gated operation to take ownership anyway.
//
// `SpyProbe` records every call (including `claim_ownership`) so tests can
// assert not just what `detect_coexistence`/`take_ownership` return, but
// that the *default*, read-only path never reaches the one write this
// module exposes.

use chimera_migration::coexistence::{
    CoexistenceCheckInput, CoexistenceError, CoexistencePort, CoexistenceVerdict, ConflictEvidence,
    LockHolder, OwnershipTakeoverConfirmation, ProbeError, RunningProcess, detect_coexistence,
    take_ownership,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct SpyProbe {
    lock_holder: Option<LockHolder>,
    ownership_owner: Option<String>,
    running_processes: Vec<RunningProcess>,
    fail_lock: bool,
    fail_ownership: bool,
    fail_processes: bool,
    fail_claim: bool,
    claim_calls: RefCell<u32>,
    last_claim: RefCell<Option<(PathBuf, String)>>,
}

impl CoexistencePort for SpyProbe {
    fn lock_holder(&self, _lock_path: &Path) -> Result<Option<LockHolder>, ProbeError> {
        if self.fail_lock {
            return Err(ProbeError::Io("simulated lock check failure".into()));
        }
        Ok(self.lock_holder.clone())
    }

    fn ownership_owner(&self, _manifest_path: &Path) -> Result<Option<String>, ProbeError> {
        if self.fail_ownership {
            return Err(ProbeError::Io("simulated manifest read failure".into()));
        }
        Ok(self.ownership_owner.clone())
    }

    fn running_manager_processes(&self) -> Result<Vec<RunningProcess>, ProbeError> {
        if self.fail_processes {
            return Err(ProbeError::Io("simulated process list failure".into()));
        }
        Ok(self.running_processes.clone())
    }

    fn claim_ownership(&self, manifest_path: &Path, owner_label: &str) -> Result<(), ProbeError> {
        *self.claim_calls.borrow_mut() += 1;
        *self.last_claim.borrow_mut() =
            Some((manifest_path.to_path_buf(), owner_label.to_string()));
        if self.fail_claim {
            return Err(ProbeError::Io("simulated claim failure".into()));
        }
        Ok(())
    }
}

fn input() -> CoexistenceCheckInput {
    CoexistenceCheckInput {
        lock_path: PathBuf::from("/fake/chimera.lock"),
        ownership_manifest_path: PathBuf::from("/fake/ownership.json"),
        expected_owner_label: "Chimera++".to_string(),
        current_pid: 4242,
    }
}

// ── Clear ────────────────────────────────────────────────────────────────────

#[test]
fn no_evidence_at_all_is_clear() {
    let probe = SpyProbe::default();

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert_eq!(verdict, CoexistenceVerdict::Clear);
}

#[test]
fn a_lock_held_by_our_own_pid_is_not_a_conflict() {
    let probe = SpyProbe {
        lock_holder: Some(LockHolder { pid: Some(4242) }),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert_eq!(verdict, CoexistenceVerdict::Clear);
}

#[test]
fn a_manifest_naming_us_is_not_a_conflict() {
    let probe = SpyProbe {
        ownership_owner: Some("Chimera++".to_string()),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert_eq!(verdict, CoexistenceVerdict::Clear);
}

#[test]
fn an_absent_manifest_is_not_evidence_of_anything() {
    let probe = SpyProbe {
        ownership_owner: None,
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert_eq!(verdict, CoexistenceVerdict::Clear);
}

// ── ConflictDetected: each evidence source, and fail-closed ambiguity ──────

#[test]
fn a_lock_held_by_another_pid_is_a_conflict() {
    let probe = SpyProbe {
        lock_holder: Some(LockHolder { pid: Some(9999) }),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    match verdict {
        CoexistenceVerdict::ConflictDetected { evidence, .. } => {
            assert_eq!(evidence.len(), 1);
            assert!(matches!(
                evidence[0],
                ConflictEvidence::LockHeld {
                    holder_pid: Some(9999),
                    ..
                }
            ));
        }
        other => panic!("expected ConflictDetected, got {other:?}"),
    }
}

#[test]
fn a_lock_held_by_an_unidentifiable_pid_is_treated_as_a_conflict_fail_closed() {
    let probe = SpyProbe {
        lock_holder: Some(LockHolder { pid: None }),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert!(
        matches!(verdict, CoexistenceVerdict::ConflictDetected { .. }),
        "an ambiguous lock holder must never be silently assumed to be us"
    );
}

#[test]
fn a_manifest_naming_another_owner_is_a_conflict() {
    let probe = SpyProbe {
        ownership_owner: Some("CC Switch".to_string()),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    match verdict {
        CoexistenceVerdict::ConflictDetected { who, evidence } => {
            assert_eq!(who, "CC Switch");
            assert!(matches!(
                evidence[0],
                ConflictEvidence::OwnershipManifestNamesAnotherOwner { .. }
            ));
        }
        other => panic!("expected ConflictDetected, got {other:?}"),
    }
}

#[test]
fn a_running_process_from_another_manager_is_a_conflict() {
    let probe = SpyProbe {
        running_processes: vec![RunningProcess {
            pid: 555,
            process_name: "OtherManager.exe".to_string(),
        }],
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    match verdict {
        CoexistenceVerdict::ConflictDetected { who, evidence } => {
            assert_eq!(who, "OtherManager.exe");
            assert!(matches!(
                evidence[0],
                ConflictEvidence::RunningProcess { .. }
            ));
        }
        other => panic!("expected ConflictDetected, got {other:?}"),
    }
}

#[test]
fn a_running_process_matching_our_own_pid_is_filtered_out_defense_in_depth() {
    // Simulates a misbehaving probe implementation that forgot to exclude
    // the caller's own process from its list.
    let probe = SpyProbe {
        running_processes: vec![RunningProcess {
            pid: 4242,
            process_name: "Chimera++.exe".to_string(),
        }],
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    assert_eq!(verdict, CoexistenceVerdict::Clear);
}

#[test]
fn every_evidence_source_firing_at_once_is_reported_together() {
    let probe = SpyProbe {
        lock_holder: Some(LockHolder { pid: Some(1) }),
        ownership_owner: Some("CC Switch".to_string()),
        running_processes: vec![RunningProcess {
            pid: 2,
            process_name: "OtherManager.exe".to_string(),
        }],
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    match verdict {
        CoexistenceVerdict::ConflictDetected { who, evidence } => {
            assert_eq!(evidence.len(), 3);
            // The manifest's named owner wins over a generic description.
            assert_eq!(who, "CC Switch");
        }
        other => panic!("expected ConflictDetected, got {other:?}"),
    }
}

#[test]
fn who_falls_back_to_a_generic_label_when_only_an_ambiguous_lock_is_evidence() {
    let probe = SpyProbe {
        lock_holder: Some(LockHolder { pid: None }),
        ..Default::default()
    };

    let verdict = detect_coexistence(&input(), &probe).unwrap();

    match verdict {
        CoexistenceVerdict::ConflictDetected { who, .. } => {
            assert_eq!(who, "an unidentified external manager");
        }
        other => panic!("expected ConflictDetected, got {other:?}"),
    }
}

// ── probe failures propagate, they are not silently treated as Clear ──────

#[test]
fn a_lock_probe_failure_is_reported_not_swallowed_as_clear() {
    let probe = SpyProbe {
        fail_lock: true,
        ..Default::default()
    };

    let result = detect_coexistence(&input(), &probe);

    assert!(matches!(result, Err(CoexistenceError::CheckFailed(_))));
}

#[test]
fn an_ownership_probe_failure_is_reported() {
    let probe = SpyProbe {
        fail_ownership: true,
        ..Default::default()
    };

    let result = detect_coexistence(&input(), &probe);

    assert!(matches!(result, Err(CoexistenceError::CheckFailed(_))));
}

#[test]
fn a_process_list_probe_failure_is_reported() {
    let probe = SpyProbe {
        fail_processes: true,
        ..Default::default()
    };

    let result = detect_coexistence(&input(), &probe);

    assert!(matches!(result, Err(CoexistenceError::CheckFailed(_))));
}

// ── the default, read-only path never takes ownership ──────────────────────

#[test]
fn the_default_detection_path_never_calls_claim_ownership() {
    // Run detect_coexistence across every scenario above (clear and several
    // conflicting ones) against one shared spy, then prove not one of them
    // ever reached the single write `CoexistencePort` exposes.
    let scenarios = vec![
        SpyProbe::default(),
        SpyProbe {
            lock_holder: Some(LockHolder { pid: Some(9999) }),
            ..Default::default()
        },
        SpyProbe {
            ownership_owner: Some("CC Switch".to_string()),
            ..Default::default()
        },
        SpyProbe {
            running_processes: vec![RunningProcess {
                pid: 1,
                process_name: "OtherManager.exe".to_string(),
            }],
            ..Default::default()
        },
    ];

    for probe in &scenarios {
        let _ = detect_coexistence(&input(), probe);
        assert_eq!(
            *probe.claim_calls.borrow(),
            0,
            "detect_coexistence must never call claim_ownership on its own"
        );
    }
}

// ── take_ownership: explicit confirmation required, verdict-gated ─────────

#[test]
fn taking_ownership_without_a_conflict_is_a_no_op_and_never_writes() {
    let probe = SpyProbe::default();

    let result = take_ownership(
        &CoexistenceVerdict::Clear,
        OwnershipTakeoverConfirmation::user_explicitly_confirmed(),
        Path::new("/fake/ownership.json"),
        "Chimera++",
        &probe,
    );

    assert!(result.is_ok());
    assert_eq!(*probe.claim_calls.borrow(), 0);
}

#[test]
fn taking_ownership_after_a_confirmed_conflict_writes_exactly_one_claim() {
    let probe = SpyProbe::default();
    let verdict = CoexistenceVerdict::ConflictDetected {
        who: "CC Switch".to_string(),
        evidence: vec![ConflictEvidence::OwnershipManifestNamesAnotherOwner {
            manifest_path: PathBuf::from("/fake/ownership.json"),
            owner: "CC Switch".to_string(),
        }],
    };

    let result = take_ownership(
        &verdict,
        OwnershipTakeoverConfirmation::user_explicitly_confirmed(),
        Path::new("/fake/ownership.json"),
        "Chimera++",
        &probe,
    );

    assert!(result.is_ok());
    assert_eq!(*probe.claim_calls.borrow(), 1);
    assert_eq!(
        *probe.last_claim.borrow(),
        Some((
            PathBuf::from("/fake/ownership.json"),
            "Chimera++".to_string()
        ))
    );
}

#[test]
fn a_claim_failure_is_reported_as_claim_failed() {
    let probe = SpyProbe {
        fail_claim: true,
        ..Default::default()
    };
    let verdict = CoexistenceVerdict::ConflictDetected {
        who: "CC Switch".to_string(),
        evidence: vec![],
    };

    let result = take_ownership(
        &verdict,
        OwnershipTakeoverConfirmation::user_explicitly_confirmed(),
        Path::new("/fake/ownership.json"),
        "Chimera++",
        &probe,
    );

    assert!(matches!(result, Err(CoexistenceError::ClaimFailed(_))));
}

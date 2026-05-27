use aenv_core::global_lock::{acquire_global_lock, release_global_lock};
use aenv_core::AenvError;

#[test]
fn lock_acquire_and_release_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("global.lock");
    let h = acquire_global_lock(&path).unwrap();
    assert!(path.exists());
    release_global_lock(h).unwrap();
    assert!(!path.exists());
}

#[test]
fn lock_rejects_when_held_by_live_pid() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("global.lock");
    let _h1 = acquire_global_lock(&path).unwrap();
    let err = acquire_global_lock(&path).unwrap_err();
    assert!(
        matches!(err, AenvError::GlobalConflict(_)),
        "expected GlobalConflict, got {err:?}"
    );
    assert_eq!(err.exit_code(), 19);
}

#[test]
fn lock_clears_stale_lock_with_dead_pid() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("global.lock");
    // Write a lock by hand pointing at a PID that surely doesn't exist
    // and at a timestamp recent enough that the STALE_SECS branch can't help.
    let stale = serde_json::json!({"pid": 4_000_000_000u32, "started_at": chrono_now()});
    std::fs::write(&path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();
    let h = acquire_global_lock(&path).unwrap();
    release_global_lock(h).unwrap();
}

#[test]
fn lock_clears_lock_older_than_five_minutes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("global.lock");
    // Use the *current* process PID so pid_alive() returns true; but a
    // started_at far in the past, so age-based staleness clears it.
    let stale = serde_json::json!({
        "pid": std::process::id(),
        "started_at": 0i64,
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();
    let h = acquire_global_lock(&path).unwrap();
    release_global_lock(h).unwrap();
}

#[test]
fn lock_clears_corrupt_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("global.lock");
    std::fs::write(&path, b"not json at all").unwrap();
    let h = acquire_global_lock(&path).unwrap();
    release_global_lock(h).unwrap();
}

// Helper so the JSON above isn't ambiguously typed.
fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

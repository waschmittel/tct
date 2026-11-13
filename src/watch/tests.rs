use super::*;
use std::time::Instant;

fn wait_for(w: &Watcher, want: Poll) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if w.poll() == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn reports_changes_below_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("boards")).unwrap();
    let w = Watcher::try_start(dir.path()).unwrap();
    assert_eq!(w.poll(), Poll::Idle);

    std::fs::write(dir.path().join("boards/card.json"), "{}").unwrap();
    assert!(wait_for(&w, Poll::Changed));
    // Consumed: quiet again.
    assert_eq!(w.poll(), Poll::Idle);
}

#[test]
fn missing_directory_fails_to_start() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Watcher::try_start(&dir.path().join("nope")).is_err());
}

#[test]
fn dead_watcher_reports_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = Watcher::try_start(dir.path()).unwrap();
    w.kill_for_test();
    assert_eq!(w.poll(), Poll::Failed);
}

use browser_security_pack::SecurityPackRunner;
use uuid::Uuid;

#[test]
fn security_pack_blocks_release_when_any_suite_fails() {
    let runner = SecurityPackRunner;
    let report = runner.run(Uuid::new_v4(), true, true, false, true, true);
    assert!(!report.is_release_allowed());
}

#[test]
fn security_pack_allows_release_when_all_pass() {
    let runner = SecurityPackRunner;
    let report = runner.run(Uuid::new_v4(), true, true, true, true, true);
    assert!(report.is_release_allowed());
}

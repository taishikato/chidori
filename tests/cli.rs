use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_mentions_agent_first_options() {
    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--max-chars"))
        .stdout(predicate::str::contains("--no-images"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn invalid_url_exits_with_code_2() {
    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg("not-a-url")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid URL"));
}

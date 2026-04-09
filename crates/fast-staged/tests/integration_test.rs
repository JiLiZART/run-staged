use std::process::Command;

#[test]
fn binary_exits_zero() {
    let status = Command::new(env!("CARGO_BIN_EXE_fast-staged"))
        .status()
        .expect("failed to run binary");
    assert!(status.success());
}

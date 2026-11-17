//! The egui-era pointer smokes were removed with that UI (FC-0022). Their flags
//! must fail before any project is loaded or window opened, never exit 0.
use std::process::Command;

#[test]
fn unported_ui_smoke_flags_fail_before_doing_any_work() {
    for flag in ["--ui-interaction-smoke", "--ui-authoring-smoke"] {
        let dir = tempfile::tempdir().expect("tempdir");
        let output_dir = dir.path().join("out");
        let output = Command::new(env!("CARGO_BIN_EXE_cloth-fit"))
            .args(["--steps", "0", flag, "--output"])
            .arg(&output_dir)
            .output()
            .expect("run cloth-fit");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{flag} exited successfully");
        assert!(
            stderr.contains(flag) && stderr.contains("not implemented"),
            "{flag} stderr: {stderr}"
        );
        assert!(
            !output_dir.exists(),
            "{flag} created output before rejecting the flag"
        );
    }
}

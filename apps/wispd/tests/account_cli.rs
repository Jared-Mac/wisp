use std::process::{Command, Stdio};

#[test]
fn help_succeeds_without_reading_stdin_or_writing_account_files() {
    let config = tempfile::tempdir().expect("temporary account config");
    for flag in ["--help", "-h"] {
        let result = Command::new(env!("CARGO_BIN_EXE_wisp-account"))
            .arg(flag)
            .env("XDG_CONFIG_HOME", config.path())
            .stdin(Stdio::null())
            .output()
            .expect("start account helper");
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("JSON request on stdin"));
        assert!(result.stderr.is_empty());
        assert_eq!(
            config
                .path()
                .read_dir()
                .expect("read config directory")
                .count(),
            0
        );
    }
}

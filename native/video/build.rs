use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/qt.cpp");
    println!("cargo:rerun-if-env-changed=QT_MOC");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let qt = pkg_config::Config::new()
        .atleast_version("6.0")
        .probe("Qt6Quick")
        .expect("Qt6 Quick development packages and pkg-config are required");
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("QT_MOC") {
        candidates.push(PathBuf::from(path));
    } else {
        for variable in ["libexecdir", "bindir"] {
            if let Ok(dir) = pkg_config::get_variable("Qt6Core", variable) {
                candidates.push(PathBuf::from(&dir).join("moc"));
                candidates.push(PathBuf::from(dir).join("libexec/moc"));
            }
        }
        candidates.extend(["moc6", "moc-qt6", "moc"].map(PathBuf::from));
    }
    let moc = candidates
        .into_iter()
        .find(|path| {
            Command::new(path).arg("-v").output().is_ok_and(|output| {
                output.status.success()
                    && (String::from_utf8_lossy(&output.stdout).contains("moc 6.")
                        || String::from_utf8_lossy(&output.stderr).contains("moc 6."))
            })
        })
        .expect("Qt6 moc not found; install Qt6 development tools or set QT_MOC");
    let mut command = Command::new(moc);
    for path in &qt.include_paths {
        command.arg(format!("-I{}", path.display()));
    }
    let status = command
        .args(["src/qt.cpp", "-o"])
        .arg(out.join("qt.moc"))
        .status()
        .unwrap();
    assert!(status.success(), "Qt meta-object generation failed");
    let mut build = cxx_build::bridge("src/lib.rs");
    build
        .file("src/qt.cpp")
        .include(out)
        .includes(qt.include_paths)
        .std("c++17")
        .flag_if_supported("-fPIC")
        .compile("wisp_video_qt");
}

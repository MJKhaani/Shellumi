fn main() {
    println!("cargo:rerun-if-changed=../../resources/shellumi.ico");
    println!("cargo:rerun-if-changed=../../resources/icon_pack/manifest.json");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
    {
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !sha.is_empty() {
                println!("cargo:rustc-env=SHELLUMI_GIT_SHA={sha}");
            }
        }
    }
    #[cfg(windows)]
    {
        use std::path::Path;
        use std::process::Command;

        let out_dir = match std::env::var("OUT_DIR") {
            Ok(dir) => dir,
            Err(_) => return,
        };
        let ico_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/shellumi.ico");

        if ico_path.exists() {
            let rc_content = format!(
                "1 ICON \"{}\"\n",
                ico_path.to_string_lossy().replace('\\', "\\\\")
            );
            let rc_file = Path::new(&out_dir).join("shellumi.rc");
            let res_file = Path::new(&out_dir).join("shellumi.res");
            if std::fs::write(&rc_file, rc_content).is_ok() {
                let status = Command::new("rc.exe")
                    .arg("/fo")
                    .arg(&res_file)
                    .arg(&rc_file)
                    .status()
                    .or_else(|_| {
                        Command::new("windres")
                            .arg("-i")
                            .arg(&rc_file)
                            .arg("-o")
                            .arg(&res_file)
                            .status()
                    });

                if let Ok(s) = status {
                    if s.success() {
                        println!("cargo:rustc-link-arg={}", res_file.display());
                    }
                }
            }
        }
    }
}

use sha2::{Digest, Sha256};
use std::{env, fs, path::Path, process::Command};

fn collect(root: &Path, path: &Path, files: &mut Vec<std::path::PathBuf>) {
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        && path.file_name().unwrap() != ".cargo"
    {
        return;
    }
    if !path.exists() {
        return;
    }
    assert!(
        !path.is_symlink(),
        "Engine source symlink is unsupported: {}",
        path.display()
    );
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read Engine source directory") {
            collect(root, &entry.expect("read source entry").path(), files);
        }
    } else if path.extension().is_some_and(|ext| {
        ["rs", "toml", "lock", "jsonc", "json", "yarn", "md"]
            .iter()
            .any(|allowed| ext == *allowed)
    }) {
        files.push(
            path.strip_prefix(root)
                .expect("workspace source")
                .to_owned(),
        );
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap().parent().unwrap();
    assert!(
        fs::read_to_string(root.join("Cargo.toml"))
            .expect("workspace manifest")
            .contains("[workspace]"),
        "tool identity requires the Engine workspace"
    );
    let mut files = Vec::new();
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
    ] {
        collect(root, &root.join(name), &mut files);
    }
    // Stories and documentation cannot change the identity of the Engine tools.
    for group in ["crates", "apps", "vendor"] {
        let directory = root.join(group);
        println!("cargo:rerun-if-changed={}", directory.display());
        for entry in fs::read_dir(directory).expect("read workspace crates") {
            let path = entry.expect("read crate entry").path();
            if path.is_dir() {
                for name in ["Cargo.toml", "build.rs", "src", "templates"] {
                    collect(root, &path.join(name), &mut files);
                }
            }
        }
    }
    files.sort();
    let mut hash = Sha256::new();
    for name in files {
        let bytes = fs::read(root.join(&name)).expect("read Engine source");
        hash.update(name.to_string_lossy().replace('\\', "/").as_bytes());
        hash.update([0]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    for name in [
        "TARGET",
        "PROFILE",
        "CARGO_ENCODED_RUSTFLAGS",
        "OPT_LEVEL",
        "DEBUG",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = env::var(name).unwrap_or_default();
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    let mut features: Vec<_> = env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_FEATURE_"))
        .collect();
    features.sort();
    for (name, value) in features {
        println!("cargo:rerun-if-env-changed={name}");
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    let rustc = Command::new(env::var_os("RUSTC").unwrap())
        .arg("-Vv")
        .output()
        .expect("rustc version");
    assert!(rustc.status.success(), "rustc version failed");
    hash.update(rustc.stdout);
    println!(
        "cargo:rustc-env=WEFTORA_TOOL_BUILD_ID={:x}",
        hash.finalize()
    );
    println!(
        "cargo:rustc-env=WEFTORA_TOOL_TARGET={}",
        env::var("TARGET").unwrap()
    );
    println!(
        "cargo:rustc-env=WEFTORA_TOOL_PROFILE={}",
        env::var("PROFILE").unwrap()
    );
}

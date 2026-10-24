use super::{CliResult, Failure};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

pub(super) fn create(directory: &Path) -> CliResult {
    fs::create_dir(directory).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            Failure::one(format!("cannot create Story: `{}` already exists; choose a new folder", directory.display()))
        } else {
            Failure::one(format!("cannot create Story folder `{}`: {error}; choose a new path in an existing writable parent directory", directory.display()))
        }
    })?;
    for (name, contents) in [
        (
            "manifest.jsonc",
            include_str!("../templates/minimal/manifest.jsonc"),
        ),
        ("main.yarn", include_str!("../templates/minimal/main.yarn")),
        ("README.md", include_str!("../templates/minimal/README.md")),
    ] {
        // Keep partial output on failure; cleanup must not delete files another process added.
        fs::OpenOptions::new().write(true).create_new(true).open(directory.join(name))
            .and_then(|mut file| file.write_all(contents.as_bytes()))
            .map_err(|error| Failure::one(format!("cannot write `{name}` in `{}`: {error}; incomplete new folder retained for inspection", directory.display())))?;
    }
    println!(
        "Created Story in `{}`. Edit manifest.jsonc and main.yarn; see README.md for check/run/export commands.",
        directory.display()
    );
    Ok(ExitCode::SUCCESS)
}

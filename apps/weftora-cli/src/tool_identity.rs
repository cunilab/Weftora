use super::Failure;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use weftora_yarn::ToolIdentity;

const MAX_IDENTITY_BYTES: u64 = 8192;
const IDENTITY_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) fn verify_player(player: &Path) -> Result<ToolIdentity, Failure> {
    let fail = |message: String| {
        Failure::one(format!(
            "error: player identity at `{}`: {message}; install the CLI and player from the same build",
            player.display()
        ))
    };
    let mut child = Command::new(player)
        .arg("--tool-identity")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| fail(format!("cannot query executable: {error}")))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    // A bounded reader also handles a child which exits but leaves its pipe open elsewhere.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(MAX_IDENTITY_BYTES + 1).read_to_end(&mut bytes);
        let _ = sender.send(result.map(|_| bytes));
    });
    let deadline = Instant::now() + IDENTITY_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail(match result {
                    Err(error) => format!("cannot wait for identity: {error}"),
                    _ => format!(
                        "identity query timed out after {} seconds",
                        IDENTITY_TIMEOUT.as_secs()
                    ),
                }));
            }
        }
    };
    if !status.success() {
        return Err(fail(format!("identity query failed with {status}")));
    }
    let bytes = receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| fail("identity output timed out".into()))?
        .map_err(|error| fail(format!("cannot read identity: {error}")))?;
    if bytes.len() > MAX_IDENTITY_BYTES as usize {
        return Err(fail("identity output exceeds 8192 bytes".into()));
    }
    let identity: ToolIdentity = serde_json::from_slice(&bytes)
        .map_err(|error| fail(format!("invalid identity JSON: {error}")))?;
    let expected = ToolIdentity::current("weftora-player");
    if identity != expected {
        return Err(fail(format!(
            "mismatched CLI/compiler/player; expected {expected:?}, got {identity:?}"
        )));
    }
    Ok(identity)
}

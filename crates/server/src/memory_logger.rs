// ---------------------------------------------------------------------------
// memory_logger.rs — server-side memory instrumentation
//
// Enabled by HARNESS_MEMORY_LOG=1 (set by `bin/harness --memory`).
// Every MEMORY_LOG_INTERVAL secs, appends a CSV line to
// ~/.homebrewagent/server_memory.log:
//
//   iso_timestamp,rss_kb,subagents
//
// Separate file from the frontend's memory.log so the two writers don't
// interleave lines. Stays silent if HARNESS_MEMORY_LOG isn't set.
// ---------------------------------------------------------------------------
// (rss obtained by shelling out to `ps` — same approach the frontend uses,
//  no new crate dependencies, portable across macOS and Linux.)
// ---------------------------------------------------------------------------

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent::subagents::SubagentRegistry;

const MEMORY_LOG_INTERVAL: Duration = Duration::from_secs(5);

fn log_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".homebrewagent")
            .join("server_memory.log"),
    )
}

fn rss_kb() -> u64 {
    let pid = std::process::id();
    let out = match Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
    {
        Ok(out) => out,
        Err(_) => return 0,
    };
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Start the periodic memory logger if `HARNESS_MEMORY_LOG=1`. Otherwise
/// returns immediately without spawning anything.
pub fn maybe_start() {
    if env::var("HARNESS_MEMORY_LOG").as_deref() != Ok("1") {
        return;
    }
    let Some(path) = log_path() else {
        eprintln!("memory_logger: $HOME not set; disabled");
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Write header (best-effort — if open fails the periodic task will
    // surface the error on its first write).
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "epoch_ms,rss_kb,subagents");
    }

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(MEMORY_LOG_INTERVAL);
        loop {
            ticker.tick().await;
            let rss = rss_kb();
            let subagents = SubagentRegistry::global().len();
            let line = format!("{},{rss},{subagents}\n", epoch_ms());

            // Open-append-close each tick so external rotation works and
            // we never hold the file open across a panic.
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
                let _ = f.write_all(line.as_bytes());
            }
        }
    });
}

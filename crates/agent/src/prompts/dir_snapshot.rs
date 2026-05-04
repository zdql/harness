// ---------------------------------------------------------------------------
// Lightweight ASCII snapshot of the current working directory
// ---------------------------------------------------------------------------

use std::path::Path;

/// Maximum number of entries (directories + files) to include in the snapshot.
const MAX_ENTRIES: usize = 50;

/// Build an ASCII listing of the top-level contents of `cwd`.
///
/// * Directories are listed first (with a trailing `/`), then files.
/// * Entries matched by the repo's `.gitignore` rules are excluded.
/// * At most [`MAX_ENTRIES`] entries are shown; a note is appended when more
///   exist.
///
/// Returns `None` if the directory can't be read or is empty after filtering.
pub fn build_dir_snapshot(cwd: &Path) -> Option<String> {
    let mut dirs: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    let read_dir = std::fs::read_dir(cwd).ok()?;

    for entry in read_dir.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden entries (., .., .git, etc.)
        if name.starts_with('.') {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        if is_dir {
            dirs.push(name.into_owned());
        } else {
            files.push(name.into_owned());
        }
    }

    // Filter out gitignored entries in one batch via `git check-ignore`.
    filter_gitignored(cwd, &mut dirs);
    filter_gitignored(cwd, &mut files);

    dirs.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    files.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));

    let total = dirs.len() + files.len();
    if total == 0 {
        return None;
    }

    let mut out = String::from("## Directory snapshot\n\n");
    out.push_str("```\n");

    let mut shown = 0;

    for d in &dirs {
        if shown >= MAX_ENTRIES {
            break;
        }
        out.push_str(d);
        out.push_str("/\n");
        shown += 1;
    }

    for f in &files {
        if shown >= MAX_ENTRIES {
            break;
        }
        out.push_str(f);
        out.push('\n');
        shown += 1;
    }

    out.push_str("```\n");

    if total > MAX_ENTRIES {
        out.push_str(&format!(
            "\n({total} entries total, showing first {MAX_ENTRIES}. \
             There may be more files deeper in the tree — this snapshot \
             is a starting point to help kickstart your initial search \
             and exploration.)\n"
        ));
    } else {
        out.push_str(
            "\n(This is a snapshot of the top-level directory to help \
             kickstart your initial search and exploration. There may be \
             additional files nested in subdirectories.)\n",
        );
    }

    Some(out)
}

/// Remove entries from `names` that `git check-ignore` reports as ignored.
fn filter_gitignored(cwd: &Path, names: &mut Vec<String>) {
    if names.is_empty() {
        return;
    }

    // Build stdin: one path per line.
    let stdin_data: String = names.iter().map(|n| format!("{n}\n")).collect();

    let result = std::process::Command::new("git")
        .args(["check-ignore", "--stdin"])
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut si) = child.stdin {
                let _ = si.write_all(stdin_data.as_bytes());
            }
            // Close stdin so the child can finish.
            drop(child.stdin.take());
            child.wait_with_output()
        });

    if let Ok(output) = result {
        let ignored: std::collections::HashSet<&str> = output
            .stdout
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
            .filter_map(|l| std::str::from_utf8(l).ok())
            .collect();

        names.retain(|n| !ignored.contains(n.as_str()));
    }
    // If git isn't available or we're outside a repo, keep everything.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_of_cwd_produces_output() {
        // Just a smoke test – won't assert exact contents since it depends
        // on the repo state, but it should produce *something* when run
        // inside the harness repo.
        let cwd = std::env::current_dir().unwrap();
        let snap = build_dir_snapshot(&cwd);
        assert!(snap.is_some());
        let text = snap.unwrap();
        assert!(text.contains("## Directory snapshot"));
        assert!(text.contains("```"));
    }
}

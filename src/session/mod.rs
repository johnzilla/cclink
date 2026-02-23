use std::time::{Duration, SystemTime};

/// Information about a discovered Claude Code session.
#[derive(Debug)]
pub struct SessionInfo {
    pub session_id: String,
    pub project: String, // cwd from JSONL progress record
    pub mtime: SystemTime,
}

/// Scan `~/.claude/projects/` for active Claude Code sessions.
///
/// Returns sessions modified within the last 24 hours, sorted by mtime
/// descending (most recent first). Returns an empty Vec if the directory
/// does not exist.
///
/// If `cwd_filter` is `Some(path)`, only sessions whose project cwd starts
/// with (or equals) the canonical form of that path are returned. Sessions
/// whose project path cannot be canonicalized (stale paths) are skipped.
pub fn discover_sessions(cwd_filter: Option<&std::path::Path>) -> anyhow::Result<Vec<SessionInfo>> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    let projects_dir = home.join(".claude/projects");

    if !projects_dir.exists() {
        return Ok(vec![]);
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(86400)) // 24-hour window for "active"
        .unwrap_or(SystemTime::UNIX_EPOCH);

    // Canonicalize the filter path once before the loop
    let canonical_filter =
        cwd_filter.map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()));

    let mut sessions: Vec<SessionInfo> = Vec::new();

    for project_dir_entry in std::fs::read_dir(&projects_dir)? {
        let project_dir = project_dir_entry?.path();
        if !project_dir.is_dir() {
            continue;
        }

        for entry in std::fs::read_dir(&project_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .jsonl files
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let mtime = entry.metadata()?.modified()?;

            // Skip sessions older than 24 hours
            if mtime < cutoff {
                continue;
            }

            // Session ID is the filename stem (the UUID)
            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if session_id.is_empty() {
                continue;
            }

            // Read cwd from JSONL progress record
            if let Ok(project) = read_session_cwd(&path) {
                // Filter by cwd if provided
                if let Some(ref filter) = canonical_filter {
                    let canonical_project = std::fs::canonicalize(&project)
                        .unwrap_or_else(|_| std::path::PathBuf::from(&project));
                    if !canonical_project.starts_with(filter) {
                        continue;
                    }
                }

                sessions.push(SessionInfo {
                    session_id,
                    project,
                    mtime,
                });
            }
        }
    }

    // Sort by mtime descending (most recent first)
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    Ok(sessions)
}

/// Read the `cwd` field from a JSONL session file.
///
/// Reads up to 20 lines and looks for the first line with a non-empty `cwd`
/// string. The second line is typically a `type=progress` record containing
/// both `cwd` and `sessionId`. Caps at 20 lines to avoid reading large files.
fn read_session_cwd(path: &std::path::Path) -> anyhow::Result<String> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {
        let line = line?;
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                if !cwd.is_empty() {
                    return Ok(cwd.to_string());
                }
            }
        }
    }

    anyhow::bail!("no cwd found in session file: {}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_sessions_returns_vec_when_no_projects_dir() {
        // discover_sessions() should never panic even when ~/.claude/projects/
        // does not exist — it returns an empty Vec instead.
        // This test is inherently environment-dependent, but the important
        // invariant is that the function returns Ok (not Err) when the
        // directory is absent.
        //
        // We can only safely test this if projects/ is absent on this machine.
        // If it IS present, the function should still return Ok.
        let result = discover_sessions(None);
        assert!(
            result.is_ok(),
            "discover_sessions must return Ok: {:?}",
            result
        );
    }

    #[test]
    fn discover_sessions_filters_by_cwd() {
        // Smoke test: passing a nonexistent path as the cwd filter must return
        // Ok with an empty Vec, since no real session can have a project path
        // that starts with a path that matches nothing on this machine.
        let result = discover_sessions(Some(std::path::Path::new(
            "/nonexistent/path/that/matches/nothing",
        )));
        assert!(
            result.is_ok(),
            "discover_sessions with cwd_filter must return Ok: {:?}",
            result
        );
        let sessions = result.unwrap();
        assert!(
            sessions.is_empty(),
            "expected empty Vec when filter matches nothing, got: {:?}",
            sessions
        );
    }
}

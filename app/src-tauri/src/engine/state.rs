/// State file management: write/parse loop state, append log entries.

use std::path::Path;

/// Write the loop state file (.loop.state) with current status.
pub fn write_state(
    dir: &Path,
    status: &str,
    cycle: u32,
    total: u32,
    errors: u32,
) -> Result<(), String> {
    let timestamp = chrono::Local::now().format("%+").to_string();
    let content = format!(
        "current_cycle={}\ntotal_cycles={}\nconsecutive_errors={}\nstatus={}\nlast_cycle_at={}\n",
        cycle, total, errors, status, timestamp
    );
    std::fs::write(dir.join(".loop.state"), content)
        .map_err(|e| format!("Failed to write state: {}", e))
}

/// Parse a .loop.state file, returning (current_cycle, total_cycles, consecutive_errors, last_cycle_at).
pub fn parse_state_file(state_file: &Path) -> (u32, u32, u32, Option<String>) {
    let content = std::fs::read_to_string(state_file).unwrap_or_default();
    let mut cc = 0u32;
    let mut tc = 0u32;
    let mut ce = 0u32;
    let mut lca = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("current_cycle=") {
            cc = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("total_cycles=") {
            tc = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("consecutive_errors=") {
            ce = val.parse().unwrap_or(0);
        }
        if let Some(val) = line.strip_prefix("last_cycle_at=") {
            lca = Some(val.to_string());
        }
    }

    (cc, tc, ce, lca)
}

/// Append a timestamped log entry to the project's auto-loop log file.
pub fn append_log(dir: &Path, message: &str) {
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let entry = format!("[{}] {}\n", timestamp, message);
    let log_path = dir.join("logs/auto-loop.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = file.write_all(entry.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_and_parse_state_roundtrip() {
        let dir = std::env::temp_dir().join("omnihive_test_state_roundtrip");
        let _ = fs::create_dir_all(&dir);

        write_state(&dir, "running", 5, 10, 2).unwrap();

        let state_file = dir.join(".loop.state");
        let (cc, tc, ce, lca) = parse_state_file(&state_file);
        assert_eq!(cc, 5);
        assert_eq!(tc, 10);
        assert_eq!(ce, 2);
        assert!(lca.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_state_file_missing() {
        let missing = Path::new("/tmp/nonexistent_omnihive_test/.loop.state");
        let (cc, tc, ce, lca) = parse_state_file(missing);
        assert_eq!(cc, 0);
        assert_eq!(tc, 0);
        assert_eq!(ce, 0);
        assert_eq!(lca, None);
    }

    #[test]
    fn test_parse_state_file_malformed() {
        let dir = std::env::temp_dir().join("omnihive_test_state_malformed");
        let _ = fs::create_dir_all(&dir);
        let state_file = dir.join(".loop.state");
        fs::write(&state_file, "current_cycle=abc\ntotal_cycles=\nrandom_line\n").unwrap();

        let (cc, tc, ce, lca) = parse_state_file(&state_file);
        assert_eq!(cc, 0); // abc can't parse
        assert_eq!(tc, 0); // empty string can't parse
        assert_eq!(ce, 0);
        assert_eq!(lca, None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_log_creates_file() {
        let dir = std::env::temp_dir().join("omnihive_test_append_log");
        let logs_dir = dir.join("logs");
        let _ = fs::create_dir_all(&logs_dir);

        append_log(&dir, "Test log message");

        let log_path = dir.join("logs/auto-loop.log");
        assert!(log_path.exists());
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Test log message"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_append_log_appends() {
        let dir = std::env::temp_dir().join("omnihive_test_append_log2");
        let logs_dir = dir.join("logs");
        let _ = fs::create_dir_all(&logs_dir);

        append_log(&dir, "First");
        append_log(&dir, "Second");

        let log_path = dir.join("logs/auto-loop.log");
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("First"));
        assert!(content.contains("Second"));

        let _ = fs::remove_dir_all(&dir);
    }
}

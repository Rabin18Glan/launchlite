use std::fs;
use std::path::Path;

pub struct ProcessMemory {
    pub rss_bytes: u64,
    pub vm_size_bytes: u64,
}

/// Reads RSS and Virtual Memory size for a given PID from /proc/{pid}/status
pub fn get_process_memory(pid: u32) -> Option<ProcessMemory> {
    let path = format!("/proc/{}/status", pid);
    read_proc_status(&path)
}

/// Reads RSS and Virtual Memory size for the current process (/proc/self/status)
pub fn get_self_memory() -> ProcessMemory {
    read_proc_status("/proc/self/status").unwrap_or(ProcessMemory {
        rss_bytes: 0,
        vm_size_bytes: 0,
    })
}

fn read_proc_status(path: &str) -> Option<ProcessMemory> {
    let content = fs::read_to_string(path).ok()?;
    let mut rss_kb = None;
    let mut vmsize_kb = None;

    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            if let Some(val) = extract_kb(line) {
                rss_kb = Some(val);
            }
        } else if line.starts_with("VmSize:") {
            if let Some(val) = extract_kb(line) {
                vmsize_kb = Some(val);
            }
        }
        if rss_kb.is_some() && vmsize_kb.is_some() {
            break;
        }
    }

    Some(ProcessMemory {
        rss_bytes: rss_kb.unwrap_or(0) * 1024,
        vm_size_bytes: vmsize_kb.unwrap_or(0) * 1024,
    })
}

fn extract_kb(line: &str) -> Option<u64> {
    let mut parts = line.split_whitespace();
    parts.next()?; // e.g. "VmRSS:"
    let val_str = parts.next()?;
    val_str.parse::<u64>().ok()
}

/// Gets the binary size of the running executable
pub fn get_binary_size() -> u64 {
    std::env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Checks if a process with the given PID is alive
pub fn is_process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

//! Processes available for attaching and adapter scenarios for them.

#[cfg(target_os = "linux")]
#[cfg(target_os = "macos")]
use pm_host::Command;

use serde_json::{Map, json};

use crate::{Adapter, Request, Scenario};

/// A running process visible to the editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Process {
    /// The operating system's process identifier.
    pub pid: u32,
    /// The program's short name.
    pub name: String,
    /// Its command line.
    pub command: String,
}

/// Lists visible processes, newest first where the platform exposes them.
pub fn processes() -> Vec<Process> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let mut found = Vec::new();
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let found = Vec::new();
    #[cfg(target_os = "linux")]
    {
        let mut started = std::collections::HashMap::new();
        if let Ok(entries) = pm_host::Host::local().fs().read_dir("/proc") {
            for entry in entries.flatten() {
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse::<u32>().ok())
                else {
                    continue;
                };
                if pid == std::process::id() {
                    continue;
                }
                let root = entry.path();
                let Ok(name) = pm_host::Host::local()
                    .fs()
                    .read_to_string(root.join("comm"))
                else {
                    continue;
                };
                let command = pm_host::Host::local()
                    .fs()
                    .read(root.join("cmdline"))
                    .map(|bytes| {
                        String::from_utf8_lossy(&bytes)
                            .replace('\0', " ")
                            .trim()
                            .to_owned()
                    })
                    .unwrap_or_default();
                let tick = pm_host::Host::local()
                    .fs()
                    .read_to_string(root.join("stat"))
                    .ok()
                    .and_then(|stat| {
                        stat.rsplit_once(')').and_then(|(_, rest)| {
                            rest.split_whitespace().nth(19)?.parse::<u64>().ok()
                        })
                    })
                    .unwrap_or_default();
                started.insert(pid, tick);
                found.push(Process {
                    pid,
                    name: name.trim().to_owned(),
                    command,
                });
            }
        }
        found.sort_by_key(|process| {
            std::cmp::Reverse((
                started.get(&process.pid).copied().unwrap_or_default(),
                process.pid,
            ))
        });
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = pm_host::Host::local()
            .command("ps")
            .args(["-axo", "pid=,comm=,args="])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let mut fields = line.split_whitespace();
                let (Some(pid), Some(name)) = (
                    fields.next().and_then(|pid| pid.parse::<u32>().ok()),
                    fields.next(),
                ) else {
                    continue;
                };
                if pid != std::process::id() {
                    found.push(Process {
                        pid,
                        name: name.to_owned(),
                        command: fields.collect::<Vec<_>>().join(" "),
                    });
                }
            }
        }
        found.sort_by_key(|process| std::cmp::Reverse(process.pid));
    }
    found
}

impl Adapter {
    /// Builds an attach scenario using this adapter's process-id convention.
    pub fn attach(self, pid: u32) -> Scenario {
        let mut config = Map::new();
        match self.id {
            "gdb" | "lldb-dap" => {
                config.insert("pid".into(), json!(pid));
            }
            "debugpy" => {
                config.insert("processId".into(), json!(pid));
            }
            "delve" => {
                config.insert("mode".into(), json!("local"));
                config.insert("processId".into(), json!(pid));
            }
            _ => {}
        }
        Scenario {
            label: format!("Attach to {pid}"),
            kind: self.id.to_owned(),
            adapter: Some(self),
            request: Request::Attach,
            before: None,
            config,
            source: None,
        }
    }
}

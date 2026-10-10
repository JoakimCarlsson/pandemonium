//! Processes available for attaching and adapter scenarios for them.

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

/// Lists visible processes on the local machine.
pub fn processes() -> Vec<Process> {
    processes_on(&pm_host::Host::local())
}

/// Lists visible processes on the machine owning the project.
pub fn processes_on(host: &pm_host::Host) -> Vec<Process> {
    let mut found = Vec::new();
    if let Ok(output) = host
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
            if !host.is_local() || pid != std::process::id() {
                found.push(Process {
                    pid,
                    name: std::path::Path::new(name)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    command: fields.collect::<Vec<_>>().join(" "),
                });
            }
        }
    }
    found.sort_by_key(|process| std::cmp::Reverse(process.pid));
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

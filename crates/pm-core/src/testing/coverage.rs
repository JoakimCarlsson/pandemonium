//! LCOV source summaries tied to the exact bytes present when read.

use pm_host::{Host, Location};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Line counts and the saved source revision of one covered file.
#[derive(Clone, Debug)]
pub struct CoveredFile {
    /// Absolute file path inside its worktree.
    pub path: PathBuf,
    /// Counts by zero-based executable line.
    pub lines: BTreeMap<usize, u64>,
    /// Source bytes to which this report was attached.
    revision: Vec<u8>,
    /// Machine holding the covered source revision.
    host: Host,
}

impl CoveredFile {
    /// Whether saved source still matches the report's captured revision.
    pub fn current(&self) -> bool {
        self.host
            .fs()
            .read(&self.path)
            .is_ok_and(|bytes| bytes == self.revision)
    }

    /// Number of executable lines hit at least once.
    pub fn covered(&self) -> usize {
        self.lines.values().filter(|count| **count > 0).count()
    }
}

/// A worktree's coverage report and revision validity.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    /// File summaries sorted by path.
    pub files: Vec<CoveredFile>,
    /// An edit or revision change has invalidated this report.
    pub stale: bool,
}

impl Coverage {
    /// Imports LCOV, rejecting malformed records and files outside the worktree.
    pub fn read(root: &Location, report: &Path) -> Result<Self, String> {
        let host = root.host.clone();
        let root = host
            .fs()
            .canonicalize(&root.path)
            .map_err(|error| error.to_string())?;
        let text = host
            .fs()
            .read_to_string(report)
            .map_err(|error| error.to_string())?;
        let mut files = BTreeMap::<PathBuf, CoveredFile>::new();
        let mut path = None;
        for line in text.lines() {
            if let Some(source) = line.strip_prefix("SF:") {
                let source = host
                    .fs()
                    .canonicalize(root.join(source))
                    .map_err(|error| error.to_string())?;
                if !source.starts_with(&root) {
                    return Err("Coverage source is outside this worktree".into());
                }
                let revision = host.fs().read(&source).map_err(|error| error.to_string())?;
                files.entry(source.clone()).or_insert(CoveredFile {
                    path: source.clone(),
                    lines: BTreeMap::new(),
                    revision,
                    host: host.clone(),
                });
                path = Some(source);
            } else if let Some(count) = line.strip_prefix("DA:") {
                let mut parts = count.split(',');
                let number: usize = parts
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| "Invalid LCOV line")?;
                let count: u64 = parts
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| "Invalid LCOV count")?;
                let file = path
                    .as_ref()
                    .and_then(|path| files.get_mut(path))
                    .ok_or("LCOV count without source")?;
                if number == 0 || number > file.revision.split(|byte| *byte == b'\n').count() {
                    return Err("LCOV line outside source".into());
                }
                *file.lines.entry(number - 1).or_default() += count;
            } else if line == "end_of_record" {
                path = None;
            }
        }
        if files.is_empty() {
            return Err("Report contains no source coverage".into());
        }
        Ok(Self {
            files: files.into_values().collect(),
            stale: false,
        })
    }

    /// Invalidates all decorations after a relevant source edit.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }
}

/// Exact saved Python sources captured before a run begins.
#[derive(Clone, Debug, Default)]
pub struct SourceRevision {
    /// Saved bytes of each discoverable Python source file.
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl SourceRevision {
    /// Captures source bytes through the shared worktree walk.
    pub fn capture(root: &Location) -> Self {
        let files = crate::walk(root)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "py"))
            .filter_map(|path| Some((path.clone(), root.host.fs().read(path).ok()?)))
            .collect();
        Self { files }
    }

    /// Whether the current saved sources are the revision the run started with.
    pub fn current(&self, root: &Location) -> bool {
        self.files == Self::capture(root).files
    }
}

//! The git state the snapshots render: a branch ahead of its remote with
//! staged, changed and untracked files, and a graph with a merged branch.

use crate::git::{ChangedFile, GitSummary, GraphCommit, GraphEdge};

/// The git panel of the sample workspace.
pub fn sample_git() -> GitSummary {
    GitSummary {
        folder: "~/Documents/repos/void".to_owned(),
        branch: "main".to_owned(),
        head: Some("4f2a9c1".to_owned()),
        upstream: Some("origin/main".to_owned()),
        ahead: 1,
        behind: 0,
        remotes: vec!["origin".to_owned()],
        repository: true,
        loaded: true,
        changes: vec![
            change("crates/void/src/reader.rs", 'M', true, SAMPLE_DIFF),
            change("crates/void/src/session.rs", 'M', false, ""),
            change("crates/void/Cargo.toml", 'M', false, ""),
            ChangedFile {
                untracked: true,
                ..change("crates/void/src/continuation.rs", '?', false, "")
            },
        ],
        selected: vec!["crates/void/src/session.rs".to_owned()],
        changes_open: true,
        graph_open: true,
        commits: sample_commits(),
        branches: vec!["main".to_owned(), "reader-continuation".to_owned()],
        remote_branches: vec!["origin/main".to_owned(), "origin/release".to_owned()],
        ..GitSummary::default()
    }
}

/// A changed file at `path` with `status`, staged when `staged`, with `diff`.
fn change(path: &str, status: char, staged: bool, diff: &str) -> ChangedFile {
    let (added, removed) = crate::git::tally(diff);
    ChangedFile {
        path: path.to_owned(),
        status,
        added: added.max(usize::from(diff.is_empty()) * 3),
        removed: removed.max(usize::from(diff.is_empty())),
        staged,
        fetched: !diff.is_empty(),
        diff: diff.to_owned(),
        ..ChangedFile::default()
    }
}

/// The diff of the staged sample file.
const SAMPLE_DIFF: &str = "@@ -12,7 +12,9 @@ impl Reader {\n     pub fn open(path: &Path) -> io::Result<Self> {\n-        let file = File::open(path)?;\n-        Ok(Self { file })\n+        let file = File::open(path)?;\n+        let position = Position::start();\n+        Ok(Self { file, position })\n     }\n \n     /// Reads the next chunk.\n-    pub fn next(&mut self) -> Option<Chunk> {\n+    pub fn next_chunk(&mut self) -> Option<Chunk> {\n+        self.position.advance();\n         self.file.read_chunk()\n     }";

/// A graph of six commits where a side branch forks and merges back.
fn sample_commits() -> Vec<GraphCommit> {
    let straight = |color| GraphEdge {
        upper: false,
        from: 0,
        to: 0,
        color,
    };
    let upper = |edge: GraphEdge| GraphEdge {
        upper: true,
        ..edge
    };
    let commit =
        |id: &str, summary: &str, refs: &[&str], lane, edges: Vec<GraphEdge>| GraphCommit {
            id: id.to_owned(),
            summary: summary.to_owned(),
            refs: refs.iter().map(|name| (*name).to_owned()).collect(),
            lane,
            color: lane,
            width: 2,
            edges,
        };
    vec![
        commit(
            "4f2a9c1",
            "Keep the reader's position across chunks",
            &["HEAD -> main"],
            0,
            vec![straight(0)],
        ),
        commit(
            "b81e044",
            "Merge branch 'reader-continuation'",
            &["origin/main"],
            0,
            vec![
                upper(straight(0)),
                straight(0),
                GraphEdge {
                    upper: false,
                    from: 0,
                    to: 1,
                    color: 1,
                },
            ],
        ),
        commit(
            "93cd7a2",
            "Continue sessions from the reader",
            &["reader-continuation"],
            1,
            vec![
                upper(straight(0)),
                straight(0),
                upper(GraphEdge {
                    upper: true,
                    from: 1,
                    to: 1,
                    color: 1,
                }),
                GraphEdge {
                    upper: false,
                    from: 1,
                    to: 0,
                    color: 1,
                },
            ],
        ),
        commit(
            "1d0be57",
            "Split the session store by responsibility",
            &[],
            0,
            vec![upper(straight(0)), straight(0)],
        ),
        commit(
            "e4471f9",
            "Add the void crate",
            &["v0.1.0"],
            0,
            vec![upper(straight(0)), straight(0)],
        ),
    ]
}

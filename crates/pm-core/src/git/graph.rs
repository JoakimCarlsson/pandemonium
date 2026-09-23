//! The lanes a commit graph is drawn in.
//!
//! Git's own `--graph` is text meant for a terminal: a row of `|`, `/` and
//! `\`, and whole rows that are nothing but lines between commits. A graph
//! drawn with shapes wants the topology instead — which lane each commit sits
//! in and which lanes run through, fork from or merge into its row — so the
//! lanes are laid out here from the parents git names, one row per commit.
//!
//! A lane keeps its column for as long as it is open. A line only bends
//! where a branch forks from or merges into a commit, which is what keeps
//! the picture readable when several branches run beside each other.

/// Which half of a commit's row a line crosses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Half {
    /// From the top edge of the row down to the commit.
    Upper,
    /// From the commit down to the bottom edge of the row.
    Lower,
}

/// One line through half of a commit's row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    /// The half of the row the line crosses.
    pub half: Half,
    /// The lane the line starts in, at the top of its half.
    pub from: usize,
    /// The lane the line ends in, at the bottom of its half.
    pub to: usize,
    /// The colour index of the branch the line belongs to.
    pub color: usize,
}

/// How one commit's row of the graph is drawn.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lanes {
    /// The lane the commit sits in.
    pub lane: usize,
    /// The colour index of the commit's own branch.
    pub color: usize,
    /// Every line crossing the row.
    pub edges: Vec<Edge>,
    /// How many lanes the row is wide.
    pub width: usize,
}

/// One lane that is open between two rows.
#[derive(Clone, Debug)]
struct Open {
    /// The commit the lane is waiting to reach.
    awaits: String,
    /// The colour index the lane was given when it opened.
    color: usize,
}

/// Lays out the lanes for commits listed children before parents.
///
/// Each entry is a commit's object name and the names of its parents, first
/// parent first. The result has one row per entry, in the same order.
pub fn lanes<'a, I>(commits: I) -> Vec<Lanes>
where
    I: IntoIterator<Item = (&'a str, &'a [String])>,
{
    let mut open: Vec<Option<Open>> = Vec::new();
    let mut colors = 0usize;
    let mut next_color = || {
        colors += 1;
        colors - 1
    };

    commits
        .into_iter()
        .map(|(id, parents)| {
            let mut edges = Vec::new();
            let awaiting: Vec<usize> = open
                .iter()
                .enumerate()
                .filter(|(_, lane)| lane.as_ref().is_some_and(|lane| lane.awaits == id))
                .map(|(index, _)| index)
                .collect();
            let lane = awaiting
                .first()
                .copied()
                .unwrap_or_else(|| vacant(&mut open));
            let color = match &open[lane] {
                Some(own) if awaiting.contains(&lane) => own.color,
                _ => next_color(),
            };

            for (index, through) in open.iter().enumerate() {
                if let Some(through) = through {
                    let to = if awaiting.contains(&index) {
                        lane
                    } else {
                        index
                    };
                    edges.push(Edge {
                        half: Half::Upper,
                        from: index,
                        to,
                        color: through.color,
                    });
                }
            }
            for &index in &awaiting {
                open[index] = None;
            }

            open[lane] = parents.first().map(|first| Open {
                awaits: first.clone(),
                color,
            });
            let mut forks = Vec::new();
            for parent in parents.iter().skip(1) {
                let existing = open
                    .iter()
                    .position(|lane| lane.as_ref().is_some_and(|lane| &lane.awaits == parent));
                let target = existing.unwrap_or_else(|| {
                    let target = vacant(&mut open);
                    open[target] = Some(Open {
                        awaits: parent.clone(),
                        color: next_color(),
                    });
                    target
                });
                forks.push((target, existing.is_none()));
            }

            for (index, below) in open.iter().enumerate() {
                let Some(below) = below else { continue };
                let fork = forks.iter().find(|(target, _)| *target == index);
                if index == lane || fork.is_some() {
                    edges.push(Edge {
                        half: Half::Lower,
                        from: lane,
                        to: index,
                        color: below.color,
                    });
                }
                if index != lane && !fork.is_some_and(|(_, fresh)| *fresh) {
                    edges.push(Edge {
                        half: Half::Lower,
                        from: index,
                        to: index,
                        color: below.color,
                    });
                }
            }
            while open.last().is_some_and(Option::is_none) {
                open.pop();
            }

            let width = edges
                .iter()
                .map(|edge| edge.from.max(edge.to) + 1)
                .chain([lane + 1])
                .max()
                .unwrap_or(1);
            Lanes {
                lane,
                color,
                edges,
                width,
            }
        })
        .collect()
}

/// The first closed lane in `open`, opening a new one at the end if none is.
fn vacant(open: &mut Vec<Option<Open>>) -> usize {
    open.iter().position(Option::is_none).unwrap_or_else(|| {
        open.push(None);
        open.len() - 1
    })
}

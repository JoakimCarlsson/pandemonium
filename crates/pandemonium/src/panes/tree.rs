//! The pane tree: what the window is divided into, and how it is divided.
//!
//! One pane is a bar of tabs with whatever is in front of them beneath it.
//! Splitting one puts two side by side, and splitting one of those again
//! nests a split inside a split, which is the whole model: a node is either
//! a pane or a division of other nodes along one axis. Nothing here knows
//! what a tab holds or how a pane is drawn — a pane names files, and the
//! store behind them says what those files are.

use std::collections::BTreeSet;

use pm_ui::{Axis, ResizePhase};

use crate::editor::FileId;
use crate::panes::saved::{Saved, SavedNode, SavedTab};

/// Smallest share of a split one pane can be dragged down to.
const MIN_SHARE: f32 = 0.05;

/// Which way a pane is divided, and which side the new pane takes.
///
/// This is the vocabulary a person has for splitting a pane — right, left,
/// up, down — rather than the one the tree has, which is an axis and a place
/// along it. [`SplitDirection::axis`] and [`SplitDirection::before`] are
/// where the two meet.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SplitDirection {
    /// The new pane opens to the left of the one being split.
    Left,
    /// The new pane opens to the right of it.
    Right,
    /// The new pane opens above it.
    Up,
    /// The new pane opens below it.
    Down,
}

impl SplitDirection {
    /// Every direction, in the order a menu offers them.
    pub const ALL: [Self; 4] = [Self::Right, Self::Left, Self::Up, Self::Down];

    /// What the direction is called where it is offered.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Split Left",
            Self::Right => "Split Right",
            Self::Up => "Split Up",
            Self::Down => "Split Down",
        }
    }

    /// The axis a split in this direction divides its panes along.
    pub const fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Up | Self::Down => Axis::Vertical,
        }
    }

    /// Whether the new pane goes before the one being split, not after it.
    pub const fn before(self) -> bool {
        matches!(self, Self::Left | Self::Up)
    }
}

/// One pane's identity for as long as it is in the tree.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PaneId(u64);

/// One split's identity for as long as it is in the tree.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SplitId(u64);

/// One pane: the tabs open in it and the one it is showing.
pub struct Pane {
    /// Which pane this is.
    id: PaneId,
    /// The files open in it, in the order their tabs are drawn.
    tabs: Vec<FileId>,
    /// The one the pane is showing.
    active: Option<FileId>,
}

impl Pane {
    /// An empty pane called `id`.
    fn new(id: PaneId) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            active: None,
        }
    }

    /// Which pane this is.
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// The files open in it, in the order their tabs are drawn.
    pub fn tabs(&self) -> &[FileId] {
        &self.tabs
    }

    /// The file the pane is showing.
    pub fn active(&self) -> Option<FileId> {
        self.active
    }

    /// Whether nothing is open in the pane.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Shows `file`, opening a tab for it when the pane has none.
    pub fn open(&mut self, file: FileId) {
        if !self.tabs.contains(&file) {
            self.tabs.push(file);
        }
        self.active = Some(file);
    }

    /// Puts `file` at `index` in the bar of tabs and shows it.
    ///
    /// A tab already in this pane is moved rather than opened twice, which
    /// is what dragging one along its own bar comes to.
    pub fn place(&mut self, file: FileId, index: usize) {
        let from = self.index_of(Some(file));
        self.tabs.retain(|open| *open != file);
        let index = match from {
            Some(from) if from < index => index - 1,
            _ => index,
        };
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, file);
        self.active = Some(file);
    }

    /// Shows `file`, if the pane has a tab for it.
    pub fn activate(&mut self, file: FileId) {
        if self.tabs.contains(&file) {
            self.active = Some(file);
        }
    }

    /// The file `steps` along the bar from the one in front, wrapping round
    /// at either end of it.
    pub fn tab_along(&self, steps: isize) -> Option<FileId> {
        if self.tabs.is_empty() {
            return None;
        }
        let count = self.tabs.len() as isize;
        let index = self.index_of(self.active).unwrap_or(0) as isize;
        self.tabs
            .get((index + steps).rem_euclid(count) as usize)
            .copied()
    }

    /// Closes the tab `file` is open in.
    pub fn close(&mut self, file: FileId) {
        self.retain(|open| open != file);
    }

    /// Keeps the tabs `keep` accepts, showing another when the front one goes.
    ///
    /// What comes forward is the tab to the right of the one that closed, as
    /// every editor with tabs does it, and the one to its left when the bar
    /// has run out on that side.
    pub fn retain(&mut self, mut keep: impl FnMut(FileId) -> bool) {
        let index = self.index_of(self.active).unwrap_or(0);
        self.tabs.retain(|file| keep(*file));
        if self.active.is_some_and(|file| self.tabs.contains(&file)) {
            return;
        }
        self.active = self
            .tabs
            .get(index.min(self.tabs.len().saturating_sub(1)))
            .copied();
    }

    /// Where `file` sits in the bar of tabs.
    fn index_of(&self, file: Option<FileId>) -> Option<usize> {
        let file = file?;
        self.tabs.iter().position(|open| *open == file)
    }
}

/// One division of the window: the nodes it holds and their shares of it.
pub struct Split {
    /// Which split this is.
    id: SplitId,
    /// The axis the children are divided along.
    axis: Axis,
    /// The children, in order along that axis.
    children: Vec<Node>,
    /// Each child's share of the split, in the same order.
    shares: Vec<f32>,
    /// The share the divider being dragged started the drag at.
    dragging: Option<f32>,
}

impl Split {
    /// Which split this is.
    pub fn id(&self) -> SplitId {
        self.id
    }

    /// The axis the children are divided along.
    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// The children, in order along that axis.
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    /// Each child's share of the split, in the same order.
    pub fn shares(&self) -> &[f32] {
        &self.shares
    }

    /// Moves the divider after `index` by `delta` of the whole split.
    ///
    /// The two panes either side of a divider trade their shares between
    /// them: everything else in the split keeps what it had, which is what
    /// makes dragging one divider leave the rest of the window where it was.
    /// Every event of a drag reports travel from the same press, so what the
    /// travel is added to is where the divider stood when the press landed.
    fn resize(&mut self, index: usize, delta: f32, phase: ResizePhase) {
        if phase == ResizePhase::Started {
            self.dragging = self.shares.get(index).copied();
        }
        let (Some(start), Some(before), Some(after)) = (
            self.dragging,
            self.shares.get(index).copied(),
            self.shares.get(index + 1).copied(),
        ) else {
            return;
        };
        let whole: f32 = self.shares.iter().sum();
        let pair = before + after;
        let min = MIN_SHARE * pair;
        let taken = (start + delta * whole).clamp(min, (pair - min).max(min));
        self.shares[index] = taken;
        self.shares[index + 1] = pair - taken;
        if phase == ResizePhase::Ended {
            self.dragging = None;
        }
    }
}

/// One node of the tree: a pane, or a division of further nodes.
pub enum Node {
    /// A pane, which is a leaf of the tree.
    Pane(Pane),
    /// A division of the space between further nodes.
    Split(Split),
}

impl Node {
    /// The pane at the far end of this node, `last` along its own axis.
    fn edge(&self, last: bool) -> Option<PaneId> {
        match (self, last) {
            (Self::Pane(pane), _) => Some(pane.id),
            (Self::Split(split), true) => split.children.last()?.edge(last),
            (Self::Split(split), false) => split.children.first()?.edge(last),
        }
    }

    /// Calls `visit` on every pane under this node, in drawing order.
    fn walk(&self, visit: &mut impl FnMut(&Pane)) {
        match self {
            Self::Pane(pane) => visit(pane),
            Self::Split(split) => {
                for child in &split.children {
                    child.walk(visit);
                }
            }
        }
    }

    /// Calls `visit` on every pane under this node, to change what it holds.
    fn walk_mut(&mut self, visit: &mut impl FnMut(&mut Pane)) {
        match self {
            Self::Pane(pane) => visit(pane),
            Self::Split(split) => {
                for child in &mut split.children {
                    child.walk_mut(visit);
                }
            }
        }
    }
}

/// How the window is divided into panes, and which of them has the keyboard.
pub struct PaneTree {
    /// The whole division, which is one pane until something is split.
    root: Node,
    /// The pane a command without a pane of its own applies to.
    focus: PaneId,
    /// The id the next pane opened will be given.
    next_pane: u64,
    /// The id the next split made will be given.
    next_split: u64,
}

impl Default for PaneTree {
    /// One empty pane, holding the keyboard.
    fn default() -> Self {
        let first = PaneId(0);
        Self {
            root: Node::Pane(Pane::new(first)),
            focus: first,
            next_pane: 1,
            next_split: 0,
        }
    }
}

impl PaneTree {
    /// The whole division of the window.
    pub fn root(&self) -> &Node {
        &self.root
    }

    /// The pane a command without a pane of its own applies to.
    pub fn focus(&self) -> PaneId {
        self.focus
    }

    /// Gives the keyboard to the pane `id` names, if the tree still has it.
    pub fn set_focus(&mut self, id: PaneId) {
        if self.path_to(id).is_some() {
            self.focus = id;
        }
    }

    /// The pane `id` names.
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        match self.at(&self.path_to(id)?)? {
            Node::Pane(pane) => Some(pane),
            Node::Split(_) => None,
        }
    }

    /// The pane `id` names, to open a tab in or close one from.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        let path = self.path_to(id)?;
        match self.at_mut(&path)? {
            Node::Pane(pane) => Some(pane),
            Node::Split(_) => None,
        }
    }

    /// The pane the keyboard is in.
    pub fn focused(&self) -> Option<&Pane> {
        self.pane(self.focus)
    }

    /// The window as it stands, in the shape a launch restores it from.
    ///
    /// A tab whose file `tab` cannot name is left out — it is one the next
    /// launch has no way of opening again — and a pane left empty by that
    /// comes back empty rather than not at all.
    pub fn save(&self, tab: &dyn Fn(FileId) -> Option<SavedTab>) -> Saved {
        Saved {
            focus: self
                .panes()
                .iter()
                .position(|pane| *pane == self.focus)
                .unwrap_or(0),
            root: written(&self.root, tab),
        }
    }

    /// The window the last launch left, with `open` opening each file again.
    ///
    /// Panes are given fresh ids as they are read: what was written down is
    /// the shape of the division and what was in it, and the identities this
    /// launch hands out are its own.
    pub fn restored(saved: &Saved, open: &mut dyn FnMut(&SavedTab) -> Option<FileId>) -> Self {
        let mut panes = 0;
        let mut splits = 0;
        let root = read(&saved.root, &mut panes, &mut splits, open);
        let mut tree = Self {
            root,
            focus: PaneId(0),
            next_pane: panes,
            next_split: splits,
        };
        tree.close_empty();
        let panes = tree.panes();
        tree.focus = panes
            .get(saved.focus)
            .or_else(|| panes.first())
            .copied()
            .unwrap_or_default();
        tree
    }

    /// Every pane of the window, in the order they are drawn.
    pub fn panes(&self) -> Vec<PaneId> {
        let mut panes = Vec::new();
        self.root.walk(&mut |pane| panes.push(pane.id));
        panes
    }

    /// Every file open in any pane of the window.
    pub fn held(&self) -> BTreeSet<FileId> {
        let mut held = BTreeSet::new();
        self.root
            .walk(&mut |pane| held.extend(pane.tabs().iter().copied()));
        held
    }

    /// Keeps in every pane the tabs `keep` accepts.
    ///
    /// This is how a file that is no longer open anywhere — a project taken
    /// out of the window, say — leaves the panes that were showing it,
    /// wherever they are in the tree.
    pub fn retain(&mut self, mut keep: impl FnMut(FileId) -> bool) {
        self.root.walk_mut(&mut |pane| pane.retain(&mut keep));
    }

    /// Closes every pane with nothing in it, save the last one standing.
    ///
    /// A pane whose last tab has closed is a division of the window with
    /// nothing to divide, so it gives its room back to its neighbour; the
    /// window itself is always one pane, empty or not.
    pub fn close_empty(&mut self) {
        while let Some(empty) = self.empty() {
            if !self.close(empty) {
                return;
            }
        }
    }

    /// A pane with nothing open in it, if the window has one.
    fn empty(&self) -> Option<PaneId> {
        let mut empty = None;
        self.root.walk(&mut |pane| {
            if empty.is_none() && pane.is_empty() {
                empty = Some(pane.id);
            }
        });
        empty
    }

    /// Whether the window is divided at all.
    pub fn is_split(&self) -> bool {
        matches!(self.root, Node::Split(_))
    }

    /// Splits the pane `id` names `direction`-ward, and names the new pane.
    ///
    /// The new pane takes half of what the old one had, so splitting a pane
    /// twice leaves three even panes rather than one large and two slivers.
    pub fn split(&mut self, id: PaneId, direction: SplitDirection) -> Option<PaneId> {
        let axis = direction.axis();
        let path = self.path_to(id)?;
        let fresh = PaneId(self.next_pane);
        self.next_pane += 1;

        let parent = path.split_last().map(|(_, parent)| parent.to_vec());
        if let (Some(parent), Some(index)) = (parent, path.last().copied())
            && let Some(Node::Split(split)) = self.at_mut(&parent)
            && split.axis == axis
        {
            let share = split.shares[index] / 2.0;
            let place = if direction.before() { index } else { index + 1 };
            split.shares[index] = share;
            split.shares.insert(place, share);
            split.children.insert(place, Node::Pane(Pane::new(fresh)));
            self.focus = fresh;
            return Some(fresh);
        }

        let id = SplitId(self.next_split);
        self.next_split += 1;
        let node = self.at_mut(&path)?;
        let taken = std::mem::replace(
            node,
            Node::Split(Split {
                id,
                axis,
                children: Vec::new(),
                shares: vec![1.0, 1.0],
                dragging: None,
            }),
        );
        let Node::Split(split) = node else {
            return None;
        };
        if direction.before() {
            split.children.push(Node::Pane(Pane::new(fresh)));
            split.children.push(taken);
        } else {
            split.children.push(taken);
            split.children.push(Node::Pane(Pane::new(fresh)));
        }
        self.focus = fresh;
        Some(fresh)
    }

    /// Closes the pane `id` names, unless it is the last one in the window.
    ///
    /// A split with one child left is no division at all, so it is replaced
    /// by that child: closing the second of two panes leaves the first where
    /// the pair was, not a split of one.
    pub fn close(&mut self, id: PaneId) -> bool {
        let Some(path) = self.path_to(id) else {
            return false;
        };
        let Some((index, parent)) = path.split_last() else {
            return false;
        };
        let (index, parent) = (*index, parent.to_vec());
        let Some(Node::Split(split)) = self.at_mut(&parent) else {
            return false;
        };

        split.children.remove(index);
        split.shares.remove(index);
        let survivor = index.min(split.children.len().saturating_sub(1));
        let focus = split
            .children
            .get(survivor)
            .and_then(|node| node.edge(false));
        if split.children.len() == 1 {
            let only = split.children.remove(0);
            if let Some(node) = self.at_mut(&parent) {
                *node = only;
            }
        }
        if let Some(focus) = focus {
            self.focus = focus;
        }
        true
    }

    /// Moves the divider after `index` in the split `id` names by `delta`.
    pub fn resize(&mut self, id: SplitId, index: usize, delta: f32, phase: ResizePhase) {
        fn find(node: &mut Node, id: SplitId) -> Option<&mut Split> {
            match node {
                Node::Pane(_) => None,
                Node::Split(split) => {
                    if split.id == id {
                        Some(split)
                    } else {
                        split.children.iter_mut().find_map(|child| find(child, id))
                    }
                }
            }
        }

        if let Some(split) = find(&mut self.root, id) {
            split.resize(index, delta, phase);
        }
    }

    /// The axis the split `id` names divides its children along.
    pub fn split_axis(&self, id: SplitId) -> Option<Axis> {
        fn find(node: &Node, id: SplitId) -> Option<Axis> {
            match node {
                Node::Pane(_) => None,
                Node::Split(split) if split.id == id => Some(split.axis),
                Node::Split(split) => split.children.iter().find_map(|child| find(child, id)),
            }
        }

        find(&self.root, id)
    }

    /// The pane next to the one `id` names, `forward` along `axis`.
    ///
    /// The neighbour is found by walking out of the pane rather than by
    /// measuring the window: the first division along that axis with
    /// something on that side is the one the focus crosses, and the pane it
    /// lands in is the one nearest the divider it crossed.
    pub fn neighbour(&self, id: PaneId, axis: Axis, forward: bool) -> Option<PaneId> {
        let mut path = self.path_to(id)?;
        while let Some(index) = path.pop() {
            let Some(Node::Split(split)) = self.at(&path) else {
                continue;
            };
            if split.axis != axis {
                continue;
            }
            let next = match (forward, index) {
                (true, index) => Some(index + 1),
                (false, 0) => None,
                (false, index) => Some(index - 1),
            };
            if let Some(pane) = next
                .and_then(|next| split.children.get(next))
                .and_then(|child| child.edge(!forward))
            {
                return Some(pane);
            }
        }
        None
    }

    /// The indices leading from the root to the pane `id` names.
    fn path_to(&self, id: PaneId) -> Option<Vec<usize>> {
        fn walk(node: &Node, id: PaneId, path: &mut Vec<usize>) -> bool {
            match node {
                Node::Pane(pane) => pane.id == id,
                Node::Split(split) => {
                    for (index, child) in split.children.iter().enumerate() {
                        path.push(index);
                        if walk(child, id, path) {
                            return true;
                        }
                        path.pop();
                    }
                    false
                }
            }
        }

        let mut path = Vec::new();
        walk(&self.root, id, &mut path).then_some(path)
    }

    /// The node `path` leads to.
    fn at(&self, path: &[usize]) -> Option<&Node> {
        let mut node = &self.root;
        for index in path {
            let Node::Split(split) = node else {
                return None;
            };
            node = split.children.get(*index)?;
        }
        Some(node)
    }

    /// The node `path` leads to, to change what is under it.
    fn at_mut(&mut self, path: &[usize]) -> Option<&mut Node> {
        let mut node = &mut self.root;
        for index in path {
            let Node::Split(split) = node else {
                return None;
            };
            node = split.children.get_mut(*index)?;
        }
        Some(node)
    }
}

/// One node of the tree, in the shape it is written down in.
fn written(node: &Node, tab: &dyn Fn(FileId) -> Option<SavedTab>) -> SavedNode {
    match node {
        Node::Pane(pane) => {
            let tabs = pane
                .tabs()
                .iter()
                .filter_map(|file| tab(*file).map(|saved| (*file, saved)))
                .collect::<Vec<_>>();
            let active = pane
                .active()
                .and_then(|active| tabs.iter().position(|(file, _)| *file == active));
            SavedNode::Pane {
                tabs: tabs.into_iter().map(|(_, saved)| saved).collect(),
                active,
            }
        }
        Node::Split(split) => SavedNode::Split {
            axis: split.axis().into(),
            shares: split.shares().to_vec(),
            children: split
                .children()
                .iter()
                .map(|child| written(child, tab))
                .collect(),
        },
    }
}

/// One node of the tree, read back out of the shape it was written in.
///
/// A division of fewer than two children is no division, so it is read as
/// whatever it held; a file that says it divides into three panes but only
/// gives two shares is given the shares it is missing, because a tree the
/// window cannot draw is worse than one it draws evenly.
fn read(
    node: &SavedNode,
    panes: &mut u64,
    splits: &mut u64,
    open: &mut dyn FnMut(&SavedTab) -> Option<FileId>,
) -> Node {
    match node {
        SavedNode::Pane { tabs, active } => {
            let id = PaneId(*panes);
            *panes += 1;
            let mut pane = Pane::new(id);
            let files = tabs.iter().map(&mut *open).collect::<Vec<_>>();
            for file in files.iter().flatten() {
                pane.open(*file);
            }
            pane.active = active
                .and_then(|active| files.get(active).copied().flatten())
                .or_else(|| pane.tabs.first().copied());
            Node::Pane(pane)
        }
        SavedNode::Split {
            axis,
            shares,
            children,
        } => {
            let mut read = children
                .iter()
                .map(|child| self::read(child, panes, splits, open))
                .collect::<Vec<_>>();
            if read.len() < 2 {
                return read.pop().unwrap_or_else(|| {
                    let id = PaneId(*panes);
                    *panes += 1;
                    Node::Pane(Pane::new(id))
                });
            }
            let id = SplitId(*splits);
            *splits += 1;
            let mut shares = shares.clone();
            shares.resize(read.len(), 1.0);
            Node::Split(Split {
                id,
                axis: (*axis).into(),
                children: read,
                shares,
                dragging: None,
            })
        }
    }
}

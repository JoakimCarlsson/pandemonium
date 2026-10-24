//! Message layout retained independently of the prompt and other workspace controls.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use pm_text::Shaper;
use pm_ui::{Node, Picture};

use crate::figures::Figures;
use crate::sample::{Message, Session, Status, Step};
use crate::state::WorkspaceState;

/// How many recently displayed message layouts survive prompt edits.
const CAPACITY: usize = 4096;

/// The source bytes retained in one message generation.
const BYTES: usize = 16 * 1024 * 1024;

/// A message and every workspace value used to build its controls and contents.
struct Cached {
    /// Its position, which determines action and link identities.
    index: usize,
    /// The current session's place in the workspace.
    page: Option<usize>,
    /// The project's path, which tool targets are shown relative to.
    project: String,
    /// The exact message, compared without cloning during input updates.
    message: Message,
    /// Whether its session is working.
    working: bool,
    /// The reply whose steps open by default.
    latest_steps: Option<usize>,
    /// Whether this message can branch into another turn.
    fork: bool,
    /// Whether it is the last message.
    last: bool,
    /// The message whose restore prompt is open.
    restoring: Option<usize>,
    /// Expanded tool cards.
    opened: Vec<(usize, usize, usize)>,
    /// Expanded markdown details.
    unfolded: Vec<(usize, usize, usize)>,
    /// Rendered figures, shared pixel buffers included.
    figures: Figures,
    /// Attachment thumbnails, shared pixel buffers included.
    previews: HashMap<String, Picture>,
    /// Fonts, width, density, text scale and running-timer second.
    geometry: (u64, u32, u32, u32, u64),
    /// The already built and laid-out message wrapper.
    node: Node,
    /// Estimated owned source and control bytes, excluding shared pixel buffers.
    bytes: usize,
}

/// Two bounded generations, indexed by session and message instead of scanning a transcript.
#[derive(Default)]
struct Cache {
    /// Recently used message layouts.
    newer: HashMap<(Option<usize>, usize), Cached>,
    /// The previous generation.
    older: HashMap<(Option<usize>, usize), Cached>,
    /// Source bytes in the newer generation.
    bytes: usize,
}

/// Counts owned strings and controls before cloning a message into the bounded cache.
fn source_bytes(message: &Message) -> usize {
    let optional = |value: &Option<String>| value.as_ref().map_or(0, String::len);
    std::mem::size_of::<Message>()
        + message.text.len()
        + optional(&message.duration)
        + message
            .said
            .iter()
            .map(|(_, text)| text.len())
            .sum::<usize>()
        + message
            .bullets
            .iter()
            .chain(&message.attachments)
            .map(String::len)
            .sum::<usize>()
        + message
            .answers
            .iter()
            .flat_map(|(_, answers)| answers)
            .map(|(question, answer)| question.len() + optional(answer))
            .sum::<usize>()
        + message
            .steps
            .iter()
            .map(|step| {
                std::mem::size_of::<Step>()
                    + match step {
                        Step::Thought(thought) => thought.text.len(),
                        Step::Plan(steps) => steps.iter().map(|step| step.text.len()).sum(),
                        Step::Tool(tool) => {
                            tool.id.len()
                                + tool.title.len()
                                + optional(&tool.target)
                                + optional(&tool.output)
                                + optional(&tool.parent)
                                + optional(&tool.agent)
                                + tool.diff.as_ref().map_or(0, |diff| {
                                    diff.path.len() + optional(&diff.old) + diff.new.len()
                                })
                        }
                    }
            })
            .sum::<usize>()
}

impl Cache {
    /// Takes a message out for validation, accounting for its source bytes.
    fn take(&mut self, key: (Option<usize>, usize)) -> Option<Cached> {
        if let Some(cached) = self.newer.remove(&key) {
            self.bytes -= cached.bytes;
            return Some(cached);
        }
        self.older.remove(&key)
    }

    /// Keeps recent messages while old sessions and streaming revisions expire.
    fn keep(&mut self, cached: Cached) {
        if self.newer.len() >= CAPACITY || self.bytes + cached.bytes > BYTES {
            self.older = std::mem::take(&mut self.newer);
            self.bytes = 0;
        }
        self.bytes += cached.bytes;
        self.newer.insert((cached.page, cached.index), cached);
    }
}

thread_local! {
    /// Recent messages, with old revisions replaced instead of accumulating during streaming.
    static CACHE: RefCell<Cache> = RefCell::default();
    /// Distinguishes message layouts from all other retained subtree keys.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// Reuses a message's construction and layout when only its surrounding input changed.
pub(super) fn message(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let message = &session.messages[index];
    let project = &state.projects[session.project].path;
    let page = state.open_index();
    let working = session.status == Status::Working;
    let latest_steps = session.latest_steps();
    let fork = session.turn_of(index) > 0;
    let last = index + 1 == session.messages.len();
    let second = if working && !message.steps.is_empty() {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_secs())
    } else {
        0
    };
    let geometry = (
        shaper.font_key(),
        width.to_bits(),
        pm_style::step_scale().to_bits(),
        pm_style::text_scale().to_bits(),
        second,
    );
    let cached = CACHE.with_borrow_mut(|cache| {
        let cached = cache.take((page, index))?;
        if cached.project == *project
            && cached.geometry == geometry
            && cached.message == *message
            && cached.working == working
            && cached.latest_steps == latest_steps
            && cached.fork == fork
            && cached.last == last
            && cached.restoring == state.restoring
            && cached.opened == state.opened
            && cached.unfolded == state.unfolded
            && cached.figures == state.figures
            && cached.previews == state.previews
        {
            let node = cached.node.clone();
            cache.keep(cached);
            Some(node)
        } else {
            None
        }
    });
    if let Some(node) = cached {
        return node;
    }
    let bytes = source_bytes(message)
        + (state.opened.len() + state.unfolded.len())
            * std::mem::size_of::<(usize, usize, usize)>();
    if bytes > 1024 * 1024 {
        return if message.from_user {
            super::user_message(state, session, index, width)
        } else {
            super::agent_message(state, session, index, width, shaper)
        };
    }
    let sequence = NEXT.with(|next| {
        let value = next.get();
        next.set(value.wrapping_add(1));
        value
    });
    let mut hash = DefaultHasher::new();
    ("conversation-message", sequence).hash(&mut hash);
    let node = pm_ui::retained(hash.finish(), width, shaper, |shaper| {
        if message.from_user {
            super::user_message(state, session, index, width)
        } else {
            super::agent_message(state, session, index, width, shaper)
        }
    });
    CACHE.with_borrow_mut(|cache| {
        cache.keep(Cached {
            index,
            page,
            project: project.clone(),
            message: message.clone(),
            working,
            latest_steps,
            fork,
            last,
            restoring: state.restoring,
            opened: state.opened.clone(),
            unfolded: state.unfolded.clone(),
            figures: state.figures.clone(),
            previews: state.previews.clone(),
            geometry,
            node: node.clone(),
            bytes,
        });
    });
    node
}

/// Retained message tests compare prompt edits and streamed changes against fresh message construction.
#[cfg(test)]
mod tests {
    use pm_renderer::DrawList;
    use pm_ui::{Input, Output, draw};

    use super::*;

    /// Captures message text and action targets without creating a window.
    fn paint(node: Node, shaper: &mut Shaper) -> Output {
        draw(
            &node,
            600.0,
            1000.0,
            &Input::default(),
            &pm_theme::default_family().dark,
            shaper,
            &mut DrawList::new(),
        )
    }

    /// Prompt edits reuse a reply, while streaming, details and geometry updates produce current contents.
    #[test]
    fn prompt_edits_do_not_stale_message_content() {
        let mut shaper = Shaper::new();
        let mut state = WorkspaceState::sample();
        let index = state.sessions[0]
            .messages
            .iter()
            .position(|message| !message.from_user)
            .unwrap();
        for change in 0..4 {
            match change {
                1 => state.prompt.apply(pm_text::Edit::Insert('x')),
                2 => {
                    let reply = &mut state.sessions[0].messages[index];
                    reply.text.push_str("\n\nNew streamed paragraph.");
                    reply.said.clear();
                }
                3 => state.unfolded.push((state.open_index().unwrap(), index, 0)),
                _ => {}
            }
            let session = &state.sessions[0];
            let cached = message(&state, session, index, 600.0, &mut shaper);
            let fresh = super::super::agent_message(&state, session, index, 600.0, &mut shaper);
            let actual = paint(cached, &mut shaper);
            let expected = paint(fresh, &mut shaper);
            let text = |output: &Output| {
                output
                    .pieces
                    .iter()
                    .map(|piece| piece.content.as_ref())
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert_eq!(text(&actual), text(&expected));
            assert_eq!(
                actual
                    .regions
                    .iter()
                    .map(|region| region.id)
                    .collect::<Vec<_>>(),
                expected
                    .regions
                    .iter()
                    .map(|region| region.id)
                    .collect::<Vec<_>>()
            );
        }
    }
}

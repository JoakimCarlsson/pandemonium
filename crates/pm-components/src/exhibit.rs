//! What an agent puts in front of the reader to look at: the card of a diagram or a prototype it showed in the conversation, and the picture of a document it wrote, a prototype or the mockup of an answer, which the window has rendered sealed from the network.

use crate::kit::{
    Flex, FocusId, IconName, Radius, Role, Styled, Tooltip, Type, column, icon_with, label,
    picture, row, sp,
};
use pm_harness::{Exhibit, ExhibitKind};
use rustle::Modifier;

use crate::button::Variant;
use crate::figures::{Drawing, Figures, Wanted, diagram_key, diagram_node, prototype_key};
use crate::inquiry::{Marks, marks_in};
use crate::parts::{clipped, glyph, line};
use crate::settings_catalog::Setting;
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Button, Markdown};

/// The first of the buttons that open a prototype in the browser pane; the exhibit numbered `n` of the message at `m` is `EXHIBIT_OPEN_BASE + m * EXHIBITS_PER_MESSAGE + n`.
pub const EXHIBIT_OPEN_BASE: u64 = 9_100_000_000_000;

/// The first of the buttons that copy the source of an exhibit, numbered as the ones that open it are.
pub const EXHIBIT_COPY_BASE: u64 = 9_200_000_000_000;

/// How many exhibits one message may have before the next message's numbering begins.
pub const EXHIBITS_PER_MESSAGE: u64 = 1000;

/// The first of the buttons that install or turn on the embedded browser from the placeholder of a prototype's picture, numbered as the ones that open the prototype are.
pub const PREVIEW_OFFER_BASE: u64 = 9_300_000_000_000;

/// The first of the buttons that install or turn on the embedded browser from the placeholder of a mockup's picture; the answer at `i` is `MOCKUP_OFFER_BASE + i`.
pub const MOCKUP_OFFER_BASE: u64 = 9_400_000_000_000;

/// The number after the last a button that installs or turns on the embedded browser may have.
const PREVIEW_OFFER_END: u64 = 9_500_000_000_000;

/// The viewport a prototype is rendered at for its card, in CSS pixels.
pub const EXHIBIT_VIEWPORT: (u32, u32) = (1024, 640);

/// The viewport the mockup of an answer is rendered at for its tile, in CSS pixels: smaller than a prototype's, so that it stays readable at the size of a tile.
pub const MOCKUP_VIEWPORT: (u32, u32) = (640, 400);

/// What a preview says in place of its picture while the embedded browser, which renders it, is not installed or is turned off.
pub const PREVIEW_NEEDS_BROWSER: &str = "Install the browser to preview";

/// What a preview says in place of its picture while the embedded browser that is installed is too old to render it, having no sealed pages.
pub const PREVIEW_NEEDS_UPDATE: &str = "Update the browser to preview";

/// What the button that opens a document live says while the embedded browser is too old to open it.
const OPEN_NEEDS_UPDATE: &str =
    "Update the browser to open prototypes · it cannot open pages sealed from the network";

/// What the button that renders a preview again after it failed says.
const RETRY_LABEL: &str = "Retry";

/// What a preview says in place of its picture while the embedded browser is installed and turned off.
const PREVIEW_NEEDS_SWITCH: &str = "Turn on the browser to preview";

/// What a preview says in place of its picture until the picture arrives.
const PREVIEW_PENDING: &str = "Rendering preview…";

/// What a download of the embedded browser says before it has said anything itself.
const HELPER_DOWNLOADING: &str = "Downloading the embedded browser…";

/// The label of the button that downloads the embedded browser.
const INSTALL_LABEL: &str = "Install browser";

/// The label of the button that downloads the embedded browser again because the installed one is too old.
const UPDATE_LABEL: &str = "Update browser";

/// The label of the button that turns the embedded browser on.
const TURN_ON_LABEL: &str = "Turn on browser";

/// What the padding and border of an exhibit's card take from its width, in logical pixels.
const CARD_INSET: f32 = 34.0;

/// A picture of a document the window is asked to have rendered, sealed from the network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewRequest {
    /// The key the picture is kept under among the figures.
    pub key: u64,
    /// The document, as HTML.
    pub html: String,
    /// The viewport's width, in CSS pixels.
    pub width: u32,
    /// The viewport's height, in CSS pixels.
    pub height: u32,
}

/// What the window knows of the helper that renders previews, the embedded browser's.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Helper {
    /// Whether this system has a helper to download at all.
    pub supported: bool,
    /// Whether the helper is installed.
    pub installed: bool,
    /// What the download of the helper says it is doing, while it runs.
    pub downloading: Option<String>,
    /// Why the last download of the helper failed, until another is started.
    pub failed: Option<String>,
    /// Whether the helper that runs said it cannot open or render documents sealed from the network, as a helper older than the window does not.
    pub outdated: bool,
}

/// What the placeholder of a preview that waits for the embedded browser offers the reader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Offer {
    /// Nothing to press: the browser is ready, or this system has none.
    Nothing,
    /// A button that downloads the browser, turning it on first when it is off.
    Install,
    /// A button that turns on the browser, which is installed.
    TurnOn,
    /// No button, and the line the running download says.
    Downloading(String),
    /// The reason the download failed, and a button that downloads again.
    Failed(String),
    /// A button that downloads the browser again because the one that runs cannot render sealed documents, with why the last update did not help, if one was tried.
    Update(Option<String>),
}

/// What the placeholder of a preview offers while the browser setting is `enabled` and the helper is as `helper` says: nothing on a system without a helper, the progress of a download that runs, a button that turns the browser on when it is installed and off, a button that updates it when it runs and is too old for sealed documents, the failure of the last download with a button to try again, and otherwise a button that installs it.
pub fn offer(enabled: bool, helper: &Helper) -> Offer {
    if !helper.supported {
        return Offer::Nothing;
    }
    if let Some(note) = helper.downloading.as_ref() {
        return Offer::Downloading(match note.trim() {
            "" => HELPER_DOWNLOADING.to_owned(),
            note => note.to_owned(),
        });
    }
    match (helper.installed, enabled, helper.failed.as_ref()) {
        (true, true, failed) if helper.outdated => Offer::Update(failed.cloned()),
        (true, true, _) => Offer::Nothing,
        (true, false, _) => Offer::TurnOn,
        (false, _, Some(reason)) => Offer::Failed(reason.clone()),
        (false, _, None) => Offer::Install,
    }
}

impl Offer {
    /// The label and the icon of the button offered, when one is.
    pub fn button(&self) -> Option<(&'static str, IconName)> {
        match self {
            Self::Nothing | Self::Downloading(_) => None,
            Self::Install => Some((INSTALL_LABEL, IconName::Globe)),
            Self::TurnOn => Some((TURN_ON_LABEL, IconName::Globe)),
            Self::Failed(_) => Some((INSTALL_LABEL, IconName::Refresh)),
            Self::Update(_) => Some((UPDATE_LABEL, IconName::Refresh)),
        }
    }

    /// The line the placeholder says beside what it offers.
    fn words(&self) -> &str {
        match self {
            Self::Nothing | Self::Install => PREVIEW_NEEDS_BROWSER,
            Self::TurnOn => PREVIEW_NEEDS_SWITCH,
            Self::Downloading(note) => note,
            Self::Failed(reason) | Self::Update(Some(reason)) => reason,
            Self::Update(None) => PREVIEW_NEEDS_UPDATE,
        }
    }

    /// What hovering over the offer says: what pressing its button does, or the whole of a line that may be cut short.
    fn hint(&self) -> String {
        match self {
            Self::Nothing => PREVIEW_NEEDS_BROWSER.to_owned(),
            Self::Install => {
                format!("{INSTALL_LABEL} · downloads the embedded browser, about 120 MB")
            }
            Self::TurnOn => {
                format!("{TURN_ON_LABEL} · the same as the switch in Settings under Browser")
            }
            Self::Downloading(note) => note.clone(),
            Self::Failed(reason) => format!("{reason} · press to download again"),
            Self::Update(None) => format!(
                "{UPDATE_LABEL} · the installed browser cannot render pages sealed from the network; downloads it again, about 120 MB"
            ),
            Self::Update(Some(reason)) => format!("{reason} · press to download again"),
        }
    }

    /// Emits what the placeholder shows for the offer in the room `form` stands for, its button answering to `id`: the line and a labelled button with its icon where words fit, the labelled button alone where only that does, and an icon alone in the narrowest; a running download has its line, or only the icon, in place of a button.
    #[track_caller]
    fn shown(&self, id: FocusId, form: Marks) {
        let tone = match self {
            Self::Failed(_) | Self::Update(Some(_)) => Role::Danger,
            _ => Role::Subtle,
        };
        let hint = self.hint();
        let button = self.button().map(|(label, icon)| match form {
            Marks::Words => Button::new(label, id)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .with_icon(icon),
            Marks::Icons => Button::new(label, id)
                .variant(Variant::Outline)
                .size(Size::Sm),
            Marks::Hidden => Button::icon(icon, id),
        });
        let spelled = form == Marks::Words || (form == Marks::Icons && button.is_none());
        let lone = !spelled && button.is_none();
        column(
            Modifier::new().fill_max_width(),
            Flex::new().center().middle().gap(2),
            || {
                if spelled {
                    caption(self.words(), tone, Modifier::new().tooltip(hint.clone()));
                }
                if let Some(button) = button {
                    button
                        .modifier(Modifier::new().tooltip(hint.clone()))
                        .build();
                }
                if lone {
                    icon_with(
                        Modifier::new().tooltip(hint.clone()),
                        IconName::Globe,
                        sp(3.5),
                        tone,
                    );
                }
            },
        );
    }
}

/// Emits what a preview that could not be rendered shows in the room `form` stands for: why, cut short where it does not fit and whole on hover, over a button answering to `id` that renders it again; the button alone where words do not fit.
#[track_caller]
fn failure(reason: &str, id: FocusId, form: Marks) {
    let button = match form {
        Marks::Hidden => Button::icon(IconName::Refresh, id),
        _ => Button::new(RETRY_LABEL, id)
            .variant(Variant::Outline)
            .size(Size::Sm)
            .with_icon(IconName::Refresh),
    };
    let hint = format!("{reason} · press to render it again");
    column(
        Modifier::new().fill_max_width(),
        Flex::new().center().middle().gap(2),
        || {
            if form == Marks::Words {
                caption(reason, Role::Danger, Modifier::new().tooltip(hint.clone()));
            }
            button.modifier(Modifier::new().tooltip(hint)).build();
        },
    );
}

/// Emits the line a placeholder says, `words` in `tone` after the globe, cut short where the placeholder is narrower than it, with `modifier` put around its box.
#[track_caller]
fn caption(words: &str, tone: Role, modifier: Modifier) {
    row(
        modifier.fill_max_width().pad_x(3),
        Flex::new().center().middle().gap(1.5),
        || {
            glyph(IconName::Globe, 3.5, tone);
            label(words, Type::new(12).ratio(1.4).tone(tone).truncate());
        },
    );
}

/// What the window still has to do for exhibits and mockups, and what it said of the helper that renders them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Exhibits {
    /// The pictures of documents to have rendered, which the window takes with [`WorkspaceState::take_preview_requests`].
    pub wanted: Vec<PreviewRequest>,
    /// The helper that renders them, as the window last said it is with [`WorkspaceState::set_browser_helper`].
    pub helper: Helper,
    /// Whether the reader asked for the helper to be downloaded, which the window takes with [`WorkspaceState::take_browser_install_asked`].
    pub install_asked: bool,
}

/// What showing `exhibit` takes, with the key its picture is kept under.
pub fn wanted_exhibit(exhibit: &Exhibit) -> (u64, Wanted) {
    match exhibit.kind {
        ExhibitKind::Diagram => (
            diagram_key(&exhibit.content),
            Wanted::Diagram(exhibit.content.clone()),
        ),
        ExhibitKind::Prototype => wanted_document(&exhibit.content, EXHIBIT_VIEWPORT),
    }
}

/// What showing the picture of the document `html` at `viewport` takes, with the key it is kept under.
pub fn wanted_document(html: &str, (width, height): (u32, u32)) -> (u64, Wanted) {
    (
        prototype_key(html, width, height),
        Wanted::Prototype {
            html: html.to_owned(),
            width,
            height,
        },
    )
}

/// `source` as a fenced block of markdown, the fence longer than any run of backticks in it so that nothing in the source ends the block.
fn fenced(source: &str) -> String {
    let longest = source
        .split(|held| held != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{}\n{fence}", source.trim_end_matches('\n'))
}

/// The picture of a document an agent wrote being described, or the quiet box of the same size that says why there is none.
pub(crate) struct Preview<'a> {
    /// What the figures hold for the document, nothing while it is being rendered.
    figure: Option<&'a Result<Drawing, String>>,
    /// The viewport the document is rendered at, in CSS pixels.
    viewport: (u32, u32),
    /// How wide the picture is, in logical pixels.
    width: f32,
    /// What the box offers while the embedded browser is missing, and what its button answers to.
    offer: Option<(&'a Offer, FocusId)>,
    /// What the caller puts around the box.
    modifier: Modifier,
    /// How the box lays out what it holds, when the caller chose.
    flex: Option<Flex>,
}

impl<'a> Preview<'a> {
    /// The picture `figure` holds of a document rendered at `viewport`, `width` logical pixels wide and as tall as `viewport` makes it, offering nothing when there is none.
    pub(crate) fn new(
        figure: Option<&'a Result<Drawing, String>>,
        viewport: (u32, u32),
        width: f32,
    ) -> Self {
        Self {
            figure,
            viewport,
            width,
            offer: None,
            modifier: Modifier::new(),
            flex: None,
        }
    }

    /// The preview offering `offer`, with a button answering to `id`, in place of a picture that waits for the embedded browser.
    pub(crate) fn offer(mut self, offer: &'a Offer, id: FocusId) -> Self {
        self.offer = Some((offer, id));
        self
    }

    /// Emits the picture, or the box that says it is being rendered, why it could not be, or what to press to have the embedded browser render it.
    #[track_caller]
    pub(crate) fn build(self) {
        let width = self.width.max(1.0).floor();
        let own = |height: f32| {
            Modifier::new()
                .width(width)
                .height(height)
                .fill_rounded(Role::Background, Radius::Lg)
                .stroke(Role::Border, Radius::Lg)
                .clip_to(Radius::Lg)
        };
        let flex = self.flex.unwrap_or(Flex::new().center().middle());
        match self.figure {
            Some(Ok(drawing)) => {
                let (across, down) = drawing.size();
                let height = (width * down / across.max(1.0)).round().max(1.0);
                row(self.modifier.chain(own(height)), flex, || {
                    picture(
                        Modifier::new().width(width).height(height),
                        &drawing.picture,
                    );
                });
            }
            other => {
                let (across, down) = self.viewport;
                let height = (width * down as f32 / across.max(1) as f32).round();
                let words = match other {
                    Some(Err(reason)) => reason.as_str(),
                    _ => PREVIEW_PENDING,
                };
                let waits = words == PREVIEW_NEEDS_BROWSER || words == PREVIEW_NEEDS_UPDATE;
                let offered = self
                    .offer
                    .filter(|(offer, _)| waits && **offer != Offer::Nothing);
                let retried = self
                    .offer
                    .filter(|(offer, _)| !waits && other.is_some() && **offer == Offer::Nothing);
                row(self.modifier.chain(own(height)), flex, || {
                    if let Some((offer, id)) = offered {
                        offer.shown(id, marks_in(width));
                    } else if let Some((_, id)) = retried {
                        failure(words, id, marks_in(width));
                    } else {
                        row(
                            Modifier::new().pad_x(3),
                            Flex::new().center().gap(1.5),
                            || {
                                glyph(IconName::Globe, 3.5, Role::Subtle);
                                line(words, 12, 1.4, Role::Subtle);
                            },
                        );
                    }
                });
            }
        }
    }
}

/// The card of an exhibit being described: what was shown, the drawings held for it, and what its buttons answer to.
pub struct ExhibitCard<'a> {
    /// The exhibit.
    exhibit: &'a Exhibit,
    /// The figures drawn so far, the exhibit's among them once it arrived.
    figures: &'a Figures,
    /// How wide the card is, in logical pixels.
    width: f32,
    /// What the button that opens a prototype in the browser pane answers to, when the card has one.
    open: Option<FocusId>,
    /// What the button that copies the exhibit's source answers to, when the card has one.
    copy: Option<FocusId>,
    /// What the placeholder of a prototype's picture offers while the embedded browser is missing, and what its button answers to.
    offer: Option<(&'a Offer, FocusId)>,
    /// What the caller puts around the card.
    modifier: Modifier,
    /// The padding the caller chose in place of the card's own, when it did.
    inner: Option<Modifier>,
    /// How the card lays out its heading and body, when the caller chose.
    flex: Option<Flex>,
}

impl<'a> ExhibitCard<'a> {
    /// The card of `exhibit`, `width` logical pixels wide, drawn from what `figures` holds for it, with no buttons.
    pub fn new(exhibit: &'a Exhibit, figures: &'a Figures, width: f32) -> Self {
        Self {
            exhibit,
            figures,
            width,
            open: None,
            copy: None,
            offer: None,
            modifier: Modifier::new(),
            inner: None,
            flex: None,
        }
    }

    /// The card with a button answering to `id` that opens a prototype, live, in the browser pane; a diagram has no such button.
    pub fn open(mut self, id: FocusId) -> Self {
        self.open = Some(id);
        self
    }

    /// The card with a button answering to `id` that copies the exhibit's source.
    pub fn copy(mut self, id: FocusId) -> Self {
        self.copy = Some(id);
        self
    }

    /// The card offering `offer`, with a button answering to `id`, where the picture of a prototype waits for the embedded browser.
    pub fn offer(mut self, offer: &'a Offer, id: FocusId) -> Self {
        self.offer = Some((offer, id));
        self
    }

    /// The card with `modifier` put around its box, outside its own border and padding.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The card with `modifier` in place of its own padding, inside its border.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = Some(modifier);
        self
    }

    /// The card laying out its heading and body by `flex` instead of its own.
    pub fn flex(mut self, flex: Flex) -> Self {
        self.flex = Some(flex);
        self
    }

    /// Emits the heading of the card: what kind of exhibit it is, its title and its buttons.
    #[track_caller]
    fn heading(&self) {
        let (icon, kind) = match self.exhibit.kind {
            ExhibitKind::Diagram => (IconName::Sparkles, "Diagram"),
            ExhibitKind::Prototype => (IconName::Globe, "Prototype"),
        };
        let title = match self.exhibit.title.trim() {
            "" => kind,
            title => title,
        };
        let outdated = self
            .offer
            .is_some_and(|(offer, _)| matches!(offer, Offer::Update(_)));
        let opens = self
            .open
            .filter(|_| self.exhibit.kind == ExhibitKind::Prototype);
        row(
            Modifier::new().fill_max_width(),
            Flex::new().center().gap(2),
            || {
                glyph(icon, 3.5, Role::Subtle);
                clipped(title, 13, Role::Text);
                label(kind, Type::new(12).ratio(1.4).tone(Role::Subtle).truncate());
                if let Some(id) = self.copy {
                    Button::icon(IconName::Copy, id)
                        .modifier(Modifier::new().tooltip("Copy source"))
                        .build();
                }
                if let Some(id) = opens {
                    Button::new("Open", id)
                        .variant(Variant::Outline)
                        .size(Size::Sm)
                        .with_icon(IconName::Maximize)
                        .disabled(outdated)
                        .modifier(Modifier::new().tooltip(if outdated {
                            OPEN_NEEDS_UPDATE
                        } else {
                            "Open live in the browser pane · sealed from the network"
                        }))
                        .build();
                }
            },
        );
    }

    /// Emits what the card shows of a diagram: the drawing once it arrived, its source as code until then, and the source with why it could not be drawn when it could not.
    #[track_caller]
    fn diagram(&self, inner: f32) {
        let source = fenced(&self.exhibit.content);
        match self.figures.get(&diagram_key(&self.exhibit.content)) {
            Some(Ok(drawing)) => diagram_node(drawing, inner),
            Some(Err(reason)) => column(Modifier::new(), Flex::new().gap(1.5), || {
                Markdown::new(&source, inner).build();
                crate::parts::paragraph(reason, 12, 1.4, Role::Danger, inner);
            }),
            None => Markdown::new(&source, inner).build(),
        }
    }

    /// Emits the body of the card: the diagram, or the picture of the prototype, `inner` logical pixels wide.
    #[track_caller]
    fn body(&self, inner: f32) {
        match self.exhibit.kind {
            ExhibitKind::Diagram => self.diagram(inner),
            ExhibitKind::Prototype => {
                let (key, _) = wanted_exhibit(self.exhibit);
                let picture = Preview::new(self.figures.get(&key), EXHIBIT_VIEWPORT, inner);
                let picture = match self.offer {
                    Some((offer, id)) => picture.offer(offer, id),
                    None => picture,
                };
                picture.build();
            }
        }
    }

    /// Emits the card: the heading over the diagram, or over the picture of the prototype.
    #[track_caller]
    pub fn build(self) {
        let inner = (self.width - CARD_INSET).max(64.0);
        let padding = self
            .inner
            .clone()
            .unwrap_or_else(|| Modifier::new().pad_x(4).pad_y(3));
        let own = Modifier::new()
            .stroke(Role::Border, Radius::Xl)
            .chain(padding);
        let flex = self.flex.unwrap_or(Flex::new().gap(2.5));
        column(self.modifier.clone().chain(own), flex, || {
            self.heading();
            self.body(inner);
        });
    }
}

/// The exhibit a button numbered `offset` past a base belongs to: the place of its message and its number among the message's exhibits.
fn exhibit_place(offset: u64) -> (usize, usize) {
    (
        (offset / EXHIBITS_PER_MESSAGE) as usize,
        (offset % EXHIBITS_PER_MESSAGE) as usize,
    )
}

/// What the button of the exhibit numbered `number` of the message at `message` answers to, counted from `base`.
pub fn exhibit_button(base: u64, message: usize, number: usize) -> FocusId {
    FocusId(base + message as u64 * EXHIBITS_PER_MESSAGE + number as u64 % EXHIBITS_PER_MESSAGE)
}

impl WorkspaceState {
    /// The exhibit of the open session a button numbered `offset` past a base belongs to, if there is one.
    fn exhibit_at(&self, offset: u64) -> Option<&Exhibit> {
        let (message, number) = exhibit_place(offset);
        self.open()?
            .messages
            .get(message)?
            .exhibits
            .get(number)
            .map(|(_, exhibit)| exhibit)
    }

    /// What pressing the button `id` does when it is one of an exhibit's of the open session, or one that installs or turns on the embedded browser, and whether it was: a prototype opens live in a tab of the session's browser, sealed from the network, and a source is put on the clipboard.
    pub(crate) fn press_exhibit(&mut self, id: FocusId) -> bool {
        let FocusId(number) = id;
        if (PREVIEW_OFFER_BASE..PREVIEW_OFFER_END).contains(&number) {
            self.press_preview_offer();
            return true;
        }
        let span = EXHIBIT_COPY_BASE - EXHIBIT_OPEN_BASE;
        let opened = number
            .checked_sub(EXHIBIT_OPEN_BASE)
            .filter(|offset| *offset < span)
            .and_then(|offset| self.exhibit_at(offset))
            .filter(|exhibit| exhibit.kind == ExhibitKind::Prototype)
            .map(|exhibit| (exhibit.title.clone(), exhibit.content.clone()));
        if let Some((title, html)) = opened {
            self.open_sealed_document(&title, &html);
            return true;
        }
        let copied = number
            .checked_sub(EXHIBIT_COPY_BASE)
            .filter(|offset| *offset < span)
            .and_then(|offset| self.exhibit_at(offset))
            .map(|exhibit| exhibit.content.clone());
        if let Some(text) = copied {
            self.outbox.push(Outgoing::Copy { text });
            return true;
        }
        false
    }

    /// Opens the document `html`, which an agent wrote, live in a tab of the browser of the session shown, called `title`: its scripts run, the network is denied to it, and agents can neither see nor drive the tab.
    pub(crate) fn open_sealed_document(&mut self, title: &str, html: &str) {
        if matches!(self.preview_offer(), Offer::Update(_)) {
            return;
        }
        let title = match title.trim() {
            "" => "Prototype",
            title => title,
        };
        self.browser_sealed_tab_opened(title, html);
    }

    /// What showing the exhibits of the session at `session`, and the mockups of the question it waits on, takes.
    fn exhibit_figures_of(&self, session: usize) -> Vec<(u64, Wanted)> {
        let Some(held) = self.sessions.get(session) else {
            return Vec::new();
        };
        let shown = held
            .messages
            .iter()
            .flat_map(|message| message.exhibits.iter())
            .map(|(_, exhibit)| wanted_exhibit(exhibit));
        let pictured = held.inquiry.iter().flat_map(|draft| {
            draft
                .inquiry
                .questions
                .iter()
                .flat_map(|question| question.alternatives.iter())
                .filter_map(|alternative| alternative.mockup.as_deref())
                .map(|html| wanted_document(html, MOCKUP_VIEWPORT))
        });
        shown.chain(pictured).collect()
    }

    /// Asks for the pictures of the exhibits of the session at `session` and of the mockups of the question it waits on that are neither drawn nor asked for yet.
    pub(crate) fn ask_exhibit_figures(&mut self, session: usize) {
        let wanted = self.exhibit_figures_of(session);
        self.ask_figures(wanted, None);
    }

    /// Asks for the pictures of the exhibits and mockups of every session, as after every figure was let go.
    pub(crate) fn ask_all_exhibit_figures(&mut self) {
        for session in 0..self.sessions.len() {
            self.ask_exhibit_figures(session);
        }
    }

    /// The pictures of documents the window has yet to have rendered, which it now is to.
    pub fn take_preview_requests(&mut self) -> Vec<PreviewRequest> {
        std::mem::take(&mut self.exhibits.wanted)
    }

    /// Whether pictures of documents wait to be rendered.
    pub fn preview_requests_pending(&self) -> bool {
        !self.exhibits.wanted.is_empty()
    }

    /// Lets go of every preview that could not be rendered because the embedded browser was missing or too old, and asks for them again, as once the browser is installed or updated.
    pub fn retry_previews(&mut self) {
        let before = self.figures.len();
        self.figures.retain(|_, drawn| {
            !matches!(drawn, Err(reason) if reason.as_str() == PREVIEW_NEEDS_BROWSER || reason.as_str() == PREVIEW_NEEDS_UPDATE)
        });
        if self.figures.len() != before {
            self.ask_all_exhibit_figures();
        }
    }

    /// Lets go of every preview of a document, in any session, that could not be rendered, whatever the reason, and asks for them again, as when the reader pressed Retry on one.
    pub fn retry_failed_previews(&mut self) {
        let documents: std::collections::HashSet<u64> = (0..self.sessions.len())
            .flat_map(|session| self.exhibit_figures_of(session))
            .filter(|(_, wanted)| matches!(wanted, Wanted::Prototype { .. }))
            .map(|(key, _)| key)
            .collect();
        let before = self.figures.len();
        self.figures
            .retain(|key, drawn| drawn.is_ok() || !documents.contains(key));
        if self.figures.len() != before {
            self.ask_all_exhibit_figures();
        }
    }

    /// What the placeholder of a preview that waits for the embedded browser offers now.
    pub fn preview_offer(&self) -> Offer {
        offer(self.window.general.embedded_browser, &self.exhibits.helper)
    }

    /// Records how the helper that renders previews is, as the window found it.
    pub fn set_browser_helper(&mut self, helper: Helper) {
        self.exhibits.helper = helper;
    }

    /// Whether the reader asked for the helper to be downloaded and the window has not taken that yet.
    pub fn browser_install_asked(&self) -> bool {
        self.exhibits.install_asked
    }

    /// Whether the reader asked for the helper to be downloaded since this was last taken.
    pub fn take_browser_install_asked(&mut self) -> bool {
        std::mem::take(&mut self.exhibits.install_asked)
    }

    /// Does what the button of a preview's placeholder offers: the browser setting is flipped on as its switch in the settings does, which downloads a missing helper, a download that failed or an update of a helper too old for sealed documents is asked for, and with the browser ready the previews that failed are rendered again.
    pub(crate) fn press_preview_offer(&mut self) {
        let offered = self.preview_offer();
        if offered == Offer::Nothing {
            self.retry_failed_previews();
            return;
        }
        if offered.button().is_none() {
            return;
        }
        if !self.window.general.embedded_browser {
            Setting::EmbeddedBrowser.press(self, 0);
        }
        self.exhibits.install_asked = offered != Offer::TurnOn;
    }
}

#[cfg(test)]
/// Tests for exhibits in a conversation and the pictures asked for them.
mod tests {
    use pm_harness::{Alternative, Event, Inquiry, Question};

    use super::*;
    use crate::state::Tab;

    /// An exhibit of `kind` holding `content`, shown by the call `id`.
    fn exhibit(id: &str, kind: ExhibitKind, content: &str) -> Exhibit {
        Exhibit {
            id: id.to_owned(),
            title: "Login flow".to_owned(),
            kind,
            content: content.to_owned(),
        }
    }

    /// The sample workspace with the first session open and its figures and requests cleared.
    fn opened() -> WorkspaceState {
        let mut state = WorkspaceState::sample();
        state.page = crate::state::Page::Session(0);
        state.figures.clear();
        state.figures_asked.clear();
        state.exhibits = Exhibits::default();
        state.outbox.clear();
        state
    }

    /// An exhibit the agent shows is kept in the reply being written, after the steps taken so far, and a prototype's picture is asked of the window once, however often it is shown.
    #[test]
    fn a_shown_prototype_is_kept_and_its_picture_asked_for_once() {
        let mut state = opened();
        let open = state.open_index().unwrap();
        let shown = exhibit("call-1", ExhibitKind::Prototype, "<p>hi</p>");
        state.hear_in_place(open, Event::Show(shown.clone()));
        state.hear_in_place(open, Event::Show(shown.clone()));
        let reply = state.sessions[open].messages.last().unwrap();
        assert_eq!(reply.exhibits.len(), 2);
        assert_eq!(reply.exhibits[0], (reply.steps.len(), shown));
        let wanted = state.take_preview_requests();
        assert_eq!(wanted.len(), 1);
        assert_eq!(
            wanted[0].key,
            prototype_key("<p>hi</p>", EXHIBIT_VIEWPORT.0, EXHIBIT_VIEWPORT.1)
        );
        assert_eq!((wanted[0].width, wanted[0].height), EXHIBIT_VIEWPORT);
        assert!(!state.preview_requests_pending());
    }

    /// A diagram is drawn by the window itself, with no browser, as a diagram of a reply is.
    #[test]
    fn a_shown_diagram_needs_no_browser() {
        let mut state = opened();
        let open = state.open_index().unwrap();
        let shown = exhibit("call-1", ExhibitKind::Diagram, "graph TD; A-->B");
        state.hear_in_place(open, Event::Show(shown));
        assert!(!state.preview_requests_pending());
        assert!(state.outbox.iter().any(|asked| matches!(
            asked,
            Outgoing::DrawDiagram { source, .. } if source == "graph TD; A-->B"
        )));
    }

    /// The same document has one key at one viewport and another at another, and a changed document another still.
    #[test]
    fn previews_are_keyed_by_document_and_viewport() {
        let key = prototype_key("<p>a</p>", 1024, 640);
        assert_eq!(key, prototype_key("<p>a</p>", 1024, 640));
        assert_ne!(key, prototype_key("<p>a</p>", 640, 400));
        assert_ne!(key, prototype_key("<p>b</p>", 1024, 640));
        assert_ne!(key, diagram_key("<p>a</p>"));
    }

    /// The mockups of a question are asked for at the viewport of a tile when the question arrives.
    #[test]
    fn the_mockups_of_a_question_are_asked_for() {
        let mut state = opened();
        let open = state.open_index().unwrap();
        let alternative = |label: &str, mockup: Option<&str>| Alternative {
            label: label.to_owned(),
            description: String::new(),
            mockup: mockup.map(str::to_owned),
        };
        state.hear_in_place(
            open,
            Event::Inquire(Inquiry {
                ticket: 1,
                questions: vec![Question {
                    header: "Layout".to_owned(),
                    text: "Which layout?".to_owned(),
                    multiple: false,
                    alternatives: vec![
                        alternative("Sidebar", Some("<p>a</p>")),
                        alternative("Tabs", Some("<p>b</p>")),
                        alternative("Neither", None),
                    ],
                }],
            }),
        );
        let wanted = state.take_preview_requests();
        assert_eq!(wanted.len(), 2);
        assert!(
            wanted
                .iter()
                .all(|asked| (asked.width, asked.height) == MOCKUP_VIEWPORT)
        );
    }

    /// The button of a prototype opens it in a sealed tab of the browser and the copy button puts the source on the clipboard; a button of no exhibit does nothing.
    #[test]
    fn the_buttons_of_an_exhibit_open_and_copy_it() {
        let mut state = opened();
        let open = state.open_index().unwrap();
        state.hear_in_place(
            open,
            Event::Show(exhibit("call-1", ExhibitKind::Prototype, "<p>hi</p>")),
        );
        let message = state.sessions[open].messages.len() - 1;
        assert!(!state.press_exhibit(exhibit_button(EXHIBIT_OPEN_BASE, message, 7)));
        assert!(state.press_exhibit(exhibit_button(EXHIBIT_COPY_BASE, message, 0)));
        assert!(state.outbox.iter().any(|asked| matches!(
            asked,
            Outgoing::Copy { text } if text == "<p>hi</p>"
        )));
        assert!(state.press_exhibit(exhibit_button(EXHIBIT_OPEN_BASE, message, 0)));
        let browser = state.browser.as_ref().unwrap();
        let sealed = browser.sealed.as_ref().unwrap();
        assert_eq!(sealed.title, "Login flow");
        assert_eq!(&*sealed.html, "<p>hi</p>");
        assert_eq!(state.tab, Tab::Browser);
    }

    /// A preview that failed for want of the browser is asked for again once the browser is there, and one that failed for another reason is not.
    #[test]
    fn previews_are_asked_for_again_once_the_browser_is_installed() {
        let mut state = opened();
        let open = state.open_index().unwrap();
        state.hear_in_place(
            open,
            Event::Show(exhibit("call-1", ExhibitKind::Prototype, "<p>hi</p>")),
        );
        let asked = state.take_preview_requests();
        state = state.with_figure(asked[0].key, Err(PREVIEW_NEEDS_BROWSER.to_owned()));
        state.retry_previews();
        assert_eq!(state.take_preview_requests().len(), 1);
        state = state.with_figure(
            asked[0].key,
            Err("The document painted nothing.".to_owned()),
        );
        state.retry_previews();
        assert!(!state.preview_requests_pending());
    }

    /// A helper this system has, installed or not, with no download running or failed.
    fn helper(installed: bool) -> Helper {
        Helper {
            supported: true,
            installed,
            downloading: None,
            failed: None,
            outdated: false,
        }
    }

    /// The placeholder offers to install a missing helper, to turn on one that is installed and off, nothing when it is ready or the system has none, the progress while it downloads and the failure with a retry once that failed.
    #[test]
    fn the_offer_follows_the_helper_and_the_setting() {
        assert_eq!(offer(false, &Helper::default()), Offer::Nothing);
        assert_eq!(offer(true, &Helper::default()), Offer::Nothing);
        assert_eq!(offer(false, &helper(false)), Offer::Install);
        assert_eq!(offer(true, &helper(false)), Offer::Install);
        assert_eq!(offer(false, &helper(true)), Offer::TurnOn);
        assert_eq!(offer(true, &helper(true)), Offer::Nothing);
        let downloading = Helper {
            downloading: Some("Unpacking the embedded browser.".to_owned()),
            failed: Some("old".to_owned()),
            ..helper(false)
        };
        assert_eq!(
            offer(true, &downloading),
            Offer::Downloading("Unpacking the embedded browser.".to_owned())
        );
        let started = Helper {
            downloading: Some(String::new()),
            ..helper(false)
        };
        assert_eq!(
            offer(true, &started),
            Offer::Downloading(HELPER_DOWNLOADING.to_owned())
        );
        let failed = Helper {
            failed: Some("no network".to_owned()),
            ..helper(false)
        };
        assert_eq!(offer(true, &failed), Offer::Failed("no network".to_owned()));
        assert_eq!(
            Offer::Install.button().map(|(label, _)| label),
            Some(INSTALL_LABEL)
        );
        assert_eq!(
            Offer::Failed(String::new())
                .button()
                .map(|(label, _)| label),
            Some(INSTALL_LABEL)
        );
        assert_eq!(
            Offer::TurnOn.button().map(|(label, _)| label),
            Some(TURN_ON_LABEL)
        );
        assert!(Offer::Nothing.button().is_none());
        assert!(Offer::Downloading(String::new()).button().is_none());
    }

    /// A helper that runs but announced no sealed pages is offered an update, with why the last update did not help once one was tried, and the buttons that open a document live are disabled; a current helper is offered nothing.
    #[test]
    fn an_outdated_helper_is_offered_an_update_and_cannot_open_documents() {
        let outdated = Helper {
            outdated: true,
            ..helper(true)
        };
        assert_eq!(offer(true, &outdated), Offer::Update(None));
        assert_eq!(offer(false, &outdated), Offer::TurnOn);
        assert_eq!(offer(true, &helper(true)), Offer::Nothing);
        let tried = Helper {
            failed: Some("still old".to_owned()),
            ..outdated.clone()
        };
        assert_eq!(
            offer(true, &tried),
            Offer::Update(Some("still old".to_owned()))
        );
        assert_eq!(
            Offer::Update(None).button().map(|(label, _)| label),
            Some(UPDATE_LABEL)
        );
        assert_eq!(Offer::Update(None).words(), PREVIEW_NEEDS_UPDATE);
        let mut state = opened();
        state.window.general.embedded_browser = true;
        state.set_browser_helper(outdated);
        assert!(state.press_exhibit(FocusId(PREVIEW_OFFER_BASE)));
        assert!(state.take_browser_install_asked());
        let tabs = state
            .browser
            .as_ref()
            .map_or(0, |browser| browser.tabs.len());
        state.open_sealed_document("Sidebar", "<p>hi</p>");
        assert_eq!(
            state
                .browser
                .as_ref()
                .map_or(0, |browser| browser.tabs.len()),
            tabs
        );
    }

    /// A preview that failed for any reason is asked for again when the reader presses Retry with the browser ready, and one that waits for an update is asked for again once the browser is updated.
    #[test]
    fn failed_previews_are_asked_for_again_on_retry() {
        let mut state = opened();
        state.window.general.embedded_browser = true;
        state.set_browser_helper(helper(true));
        let open = state.open_index().unwrap();
        state.hear_in_place(
            open,
            Event::Show(exhibit("show-1", ExhibitKind::Prototype, "<p>hi</p>")),
        );
        let asked = state.take_preview_requests();
        assert_eq!(asked.len(), 1);
        state = state.with_figure(asked[0].key, Err("The browser stopped.".to_owned()));
        assert!(state.press_exhibit(FocusId(PREVIEW_OFFER_BASE)));
        assert_eq!(state.take_preview_requests(), asked);
        state = state.with_figure(asked[0].key, Err(PREVIEW_NEEDS_UPDATE.to_owned()));
        state.retry_previews();
        assert_eq!(state.take_preview_requests(), asked);
    }

    /// Pressing the placeholder's button turns the browser setting on and, when the helper is missing, asks the window to download it; with nothing offered it does nothing.
    #[test]
    fn pressing_the_offer_turns_the_browser_on_and_asks_for_the_download() {
        let mut state = opened();
        state.window.general.embedded_browser = false;
        assert!(state.press_exhibit(FocusId(PREVIEW_OFFER_BASE)));
        assert!(!state.window.general.embedded_browser);
        assert!(!state.take_browser_install_asked());
        state.set_browser_helper(helper(true));
        assert!(state.press_exhibit(FocusId(MOCKUP_OFFER_BASE + 2)));
        assert!(state.window.general.embedded_browser);
        assert!(!state.take_browser_install_asked());
        state.window.general.embedded_browser = false;
        state.set_browser_helper(helper(false));
        assert!(state.press_exhibit(exhibit_button(PREVIEW_OFFER_BASE, 3, 1)));
        assert!(state.window.general.embedded_browser);
        assert!(state.take_browser_install_asked());
        assert!(!state.browser_install_asked());
        state.set_browser_helper(Helper {
            failed: Some("no network".to_owned()),
            ..helper(false)
        });
        assert!(state.press_exhibit(FocusId(PREVIEW_OFFER_BASE)));
        assert!(state.window.general.embedded_browser);
        assert!(state.take_browser_install_asked());
    }

    /// A fence is longer than any run of backticks in what it holds.
    #[test]
    fn a_fence_outlasts_the_backticks_inside_it() {
        assert!(fenced("a").starts_with("```text\n"));
        assert!(fenced("a ```` b").starts_with("`````text\n"));
    }
}

//! A question the agent put to the reader, and what has been filled in so far.

use pm_acp::{Alternative, Elicitation, Field, Given, Input, Inquiry, Link, Reply};

use crate::input::Input as TextBox;

/// An [`Elicitation`] waiting on the reader, with the answers given to it so far.
pub struct Form {
    /// What the agent asked.
    elicitation: Elicitation,
    /// The answer to each field, in the order the form lists them.
    given: Vec<Option<Given>>,
    /// The box each field's line of text or number is written in, in the
    /// order the form lists them; a field of another kind leaves its box empty.
    boxes: Vec<TextBox>,
    /// Whether the reader has picked each field's row of their own words.
    picked: Vec<bool>,
    /// The page the reader is looking at.
    page: usize,
    /// The row of the page the keyboard is on.
    cursor: usize,
    /// Whether the form is folded down to its tabs.
    folded: bool,
}

/// One question of a form, shown as a tab of its own.
///
/// A line of text right after a choice is that choice's own answer in other
/// words: it is shown on the choice's page, as the row the reader writes
/// their own answer in, rather than as a question of its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Page {
    /// The field the page asks.
    pub field: usize,
    /// The line of text that answers it in the reader's own words, if any.
    pub other: Option<usize>,
}

/// One row of the page in front, in the order it is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormRow {
    /// The alternative in the second place, of the field in the first.
    Choice(usize, usize),
    /// The box the field in this place is written in.
    Own(usize),
    /// Send the form as it is filled in.
    Submit,
    /// Visit the page the agent sent the reader to.
    Link,
}

impl Form {
    /// A form for `elicitation`, with every field holding the default the agent offered.
    pub fn new(elicitation: Elicitation) -> Self {
        let given = match &elicitation.inquiry {
            Inquiry::Form(fields) => fields.iter().map(preset).collect(),
            Inquiry::Link(_) => Vec::new(),
        };
        let boxes = match &elicitation.inquiry {
            Inquiry::Form(fields) => fields.iter().map(text_box).collect(),
            Inquiry::Link(_) => Vec::new(),
        };
        let picked = vec![false; boxes.len()];
        Self {
            elicitation,
            given,
            boxes,
            picked,
            page: 0,
            cursor: 0,
            folded: false,
        }
    }

    /// The ticket the reply is given under.
    pub fn id(&self) -> u64 {
        self.elicitation.id
    }

    /// What the agent says it is asking for.
    pub fn message(&self) -> &str {
        &self.elicitation.message
    }

    /// The page the reader is asked to visit, when the agent asked for one.
    pub fn link(&self) -> Option<&Link> {
        match &self.elicitation.inquiry {
            Inquiry::Link(link) => Some(link),
            Inquiry::Form(_) => None,
        }
    }

    /// What the form asks for, in order.
    pub fn fields(&self) -> &[Field] {
        match &self.elicitation.inquiry {
            Inquiry::Form(fields) => fields,
            Inquiry::Link(_) => &[],
        }
    }

    /// The questions of the form, a page each, in order.
    pub fn pages(&self) -> Vec<Page> {
        let mut pages: Vec<Page> = Vec::new();
        for (place, field) in self.fields().iter().enumerate() {
            let previous = pages.last_mut().filter(|page| {
                page.other.is_none()
                    && matches!(
                        self.fields()[page.field].input,
                        Input::One { .. } | Input::Many { .. }
                    )
            });
            match (&field.input, previous) {
                (Input::Text { .. }, Some(page)) => page.other = Some(place),
                _ => pages.push(Page {
                    field: place,
                    other: None,
                }),
            }
        }
        pages
    }

    /// The place of the page the reader is looking at.
    pub fn page(&self) -> usize {
        self.page
    }

    /// Turns to the `page`-th page.
    pub fn show_page(&mut self, page: usize) {
        self.page = page.min(self.pages().len().saturating_sub(1));
        self.cursor = 0;
        self.folded = false;
    }

    /// Turns `by` pages along, round from the last to the first.
    pub fn turn(&mut self, by: isize) {
        let count = self.pages().len().max(1) as isize;
        self.show_page((self.page as isize + by).rem_euclid(count) as usize);
    }

    /// The rows of the page in front, in the order they are drawn.
    pub fn rows(&self) -> Vec<FormRow> {
        if self.link().is_some() {
            return vec![FormRow::Link];
        }
        let Some(page) = self.pages().get(self.page).copied() else {
            return vec![FormRow::Submit];
        };
        let mut rows = match &self.fields()[page.field].input {
            Input::One { options, .. } | Input::Many { options, .. } => (0..options.len())
                .map(|option| FormRow::Choice(page.field, option))
                .collect(),
            Input::Toggle { .. } => vec![
                FormRow::Choice(page.field, 0),
                FormRow::Choice(page.field, 1),
            ],
            Input::Text { .. } | Input::Number { .. } => vec![FormRow::Own(page.field)],
        };
        rows.extend(page.other.map(FormRow::Own));
        rows.push(FormRow::Submit);
        rows
    }

    /// The row of the page the keyboard is on.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the keyboard `by` rows, round from the last to the first.
    pub fn step(&mut self, by: isize) {
        let count = self.rows().len().max(1) as isize;
        self.cursor = (self.cursor as isize + by).rem_euclid(count) as usize;
    }

    /// Puts the keyboard on the `row`-th row.
    pub fn point_at(&mut self, row: usize) {
        self.cursor = row.min(self.rows().len().saturating_sub(1));
    }

    /// What the alternative the keyboard is on looks like, or else the one
    /// chosen on this page, where the agent drew it one.
    pub fn preview(&self) -> Option<&str> {
        let lit = match self.rows().get(self.cursor) {
            Some(FormRow::Choice(place, option)) => self.alternative(*place, *option),
            _ => None,
        };
        let page = self.pages().get(self.page).copied()?;
        let chosen = || {
            (0..self.alternatives(page.field).len())
                .filter(|option| self.chosen(page.field, *option))
                .find_map(|option| self.alternative(page.field, option))
                .filter(|alternative| alternative.preview.is_some())
        };
        lit.filter(|alternative| alternative.preview.is_some())
            .or_else(chosen)?
            .preview
            .as_deref()
    }

    /// The alternatives the field at `place` offers.
    fn alternatives(&self, place: usize) -> &[Alternative] {
        match self.fields().get(place).map(|field| &field.input) {
            Some(Input::One { options, .. } | Input::Many { options, .. }) => options,
            _ => &[],
        }
    }

    /// The `option`-th alternative the field at `place` offers.
    fn alternative(&self, place: usize, option: usize) -> Option<&Alternative> {
        self.alternatives(place).get(option)
    }

    /// Whether the form is folded down to its tabs.
    pub fn folded(&self) -> bool {
        self.folded
    }

    /// Folds the form down to its tabs, or opens it again.
    pub fn fold(&mut self) {
        self.folded = !self.folded;
    }

    /// Whether the `option`-th alternative of the field at `place` is chosen.
    ///
    /// A yes-or-no is a choice of two: yes first, then no.
    pub fn chosen(&self, place: usize, option: usize) -> bool {
        let Some(field) = self.fields().get(place) else {
            return false;
        };
        match (&field.input, self.given(place)) {
            (Input::One { options, .. }, Some(Given::Chosen(value))) => options
                .get(option)
                .is_some_and(|alternative| alternative.value == *value),
            (Input::Many { options, .. }, Some(Given::ChosenMany(values))) => options
                .get(option)
                .is_some_and(|alternative| values.contains(&alternative.value)),
            (Input::Toggle { .. }, Some(Given::Flag(flag))) => *flag == (option == 0),
            _ => false,
        }
    }

    /// The page whose own-words answer is the field at `place`.
    fn owner_of(&self, place: usize) -> Option<Page> {
        self.pages()
            .into_iter()
            .find(|page| page.other == Some(place))
    }

    /// Whether the field at `place` takes exactly one answer.
    fn takes_one(&self, place: usize) -> bool {
        matches!(
            self.fields().get(place).map(|field| &field.input),
            Some(Input::One { .. } | Input::Toggle { .. })
        )
    }

    /// Turns to the page after the one the reader is on, if there is one.
    ///
    /// On the last page there is nothing after it, and the keyboard goes to
    /// the row that sends the form instead.
    fn advance(&mut self) {
        match self.page + 1 < self.pages().len() {
            true => self.show_page(self.page + 1),
            false => self.cursor = self.rows().len().saturating_sub(1),
        }
    }

    /// What the `place`-th field is filled in with now.
    pub fn given(&self, place: usize) -> Option<&Given> {
        self.given.get(place).and_then(Option::as_ref)
    }

    /// The box the `place`-th field is written in.
    pub fn text_box(&self, place: usize) -> Option<&TextBox> {
        self.boxes.get(place)
    }

    /// The box the `place`-th field is written in, to write in.
    pub fn text_box_mut(&mut self, place: usize) -> Option<&mut TextBox> {
        self.boxes.get_mut(place)
    }

    /// Whether the reader has picked the `place`-th field, the row of their
    /// own words, or written something in it.
    pub fn other_picked(&self, place: usize) -> bool {
        self.picked.get(place).copied().unwrap_or(false)
            || self.text_box(place).is_some_and(|text| !text.is_empty())
    }

    /// Picks the row of the reader's own words at `place`, letting go of
    /// whatever was chosen in its place when its question takes one answer.
    pub fn pick_other(&mut self, place: usize) {
        let Some(picked) = self.picked.get_mut(place) else {
            return;
        };
        *picked = true;
        if let Some(owner) = self.owner_of(place)
            && self.takes_one(owner.field)
        {
            self.given[owner.field] = None;
        }
    }

    /// Whether anything has been answered yet.
    pub fn answered(&self) -> bool {
        self.given.iter().any(Option::is_some) || self.boxes.iter().any(|text| !text.is_empty())
    }

    /// Fills every line of text and number in from its box, or says why one will not take it.
    pub fn settle(&mut self) -> Result<(), String> {
        for place in 0..self.boxes.len() {
            if matches!(
                self.fields()[place].input,
                Input::Text { .. } | Input::Number { .. }
            ) {
                let typed = self.boxes[place].value();
                self.enter(place, &typed)?;
            }
        }
        Ok(())
    }

    /// Fills the `place`-th field in with the line `typed`, or says why it will not take it.
    ///
    /// A blank line empties the field.
    fn enter(&mut self, place: usize, typed: &str) -> Result<(), String> {
        let Some(field) = self.fields().get(place) else {
            return Ok(());
        };
        let typed = typed.trim();
        let given = match (&field.input, typed.is_empty()) {
            (_, true) => None,
            (Input::Text { min, max, .. }, false) => {
                let length = typed.chars().count();
                if min.is_some_and(|min| length < min) {
                    return Err(format!(
                        "{} needs at least {} characters",
                        field.title,
                        min.unwrap_or(0)
                    ));
                }
                if max.is_some_and(|max| length > max) {
                    return Err(format!(
                        "{} takes at most {} characters",
                        field.title,
                        max.unwrap_or(0)
                    ));
                }
                Some(Given::Text(typed.to_owned()))
            }
            (
                Input::Number {
                    whole, min, max, ..
                },
                false,
            ) => {
                let number = typed
                    .parse::<f64>()
                    .ok()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| format!("{typed} is not a number"))?;
                if *whole && number.fract() != 0.0 {
                    return Err(format!("{} takes a whole number", field.title));
                }
                if min.is_some_and(|min| number < min) {
                    return Err(format!(
                        "{} is at least {}",
                        field.title,
                        min.unwrap_or(0.0)
                    ));
                }
                if max.is_some_and(|max| number > max) {
                    return Err(format!("{} is at most {}", field.title, max.unwrap_or(0.0)));
                }
                Some(Given::Number(number))
            }
            _ => return Ok(()),
        };
        if let Some(owner) = self.owner_of(place).filter(|_| given.is_some())
            && self.takes_one(owner.field)
        {
            self.given[owner.field] = None;
        }
        self.given[place] = given;
        Ok(())
    }

    /// Chooses the `option`-th alternative of the field at `place`.
    ///
    /// A field of one choice takes it in place of the last, and of anything
    /// written in its own words, and turns to the next page; a field of many
    /// adds it, or takes it away where it was already chosen.
    pub fn choose(&mut self, place: usize, option: usize) {
        let Some(field) = self.fields().get(place) else {
            return;
        };
        match &field.input {
            Input::One { options, .. } => {
                let Some(chosen) = options.get(option).map(|option| option.value.clone()) else {
                    return;
                };
                self.given[place] = Some(Given::Chosen(chosen));
                if let Some(other) = self
                    .pages()
                    .into_iter()
                    .find(|page| page.field == place)
                    .and_then(|page| page.other)
                {
                    self.given[other] = None;
                    self.picked[other] = false;
                    self.boxes[other].clear();
                }
                self.advance();
            }
            Input::Toggle { .. } => {
                self.given[place] = Some(Given::Flag(option == 0));
                self.advance();
            }
            Input::Many { options, .. } => {
                let Some(chosen) = options.get(option).map(|option| option.value.clone()) else {
                    return;
                };
                let mut values = match self.given(place) {
                    Some(Given::ChosenMany(values)) => values.clone(),
                    _ => Vec::new(),
                };
                match values.iter().position(|value| *value == chosen) {
                    Some(at) => {
                        values.remove(at);
                    }
                    None => values.push(chosen),
                }
                self.given[place] = Some(Given::ChosenMany(values));
            }
            _ => {}
        }
    }

    /// What is wrong with the form as it stands, or `None` when it can be sent.
    pub fn problem(&self) -> Option<String> {
        self.fields().iter().enumerate().find_map(|(place, field)| {
            let given = self.given(place);
            if field.required && given.is_none() {
                return Some(format!("{} is required", field.title));
            }
            let (Input::Many { min, max, .. }, Some(Given::ChosenMany(values))) =
                (&field.input, given)
            else {
                return None;
            };
            if min.is_some_and(|min| values.len() < min) {
                return Some(format!(
                    "{} needs at least {} chosen",
                    field.title,
                    min.unwrap_or(0)
                ));
            }
            if max.is_some_and(|max| values.len() > max) {
                return Some(format!(
                    "{} takes at most {} chosen",
                    field.title,
                    max.unwrap_or(0)
                ));
            }
            None
        })
    }

    /// The reply that sends what has been filled in.
    pub fn submission(&self) -> Reply {
        Reply::Accept(
            self.fields()
                .iter()
                .zip(&self.given)
                .filter_map(|(field, given)| Some((field.name.clone(), given.clone()?)))
                .collect(),
        )
    }
}

/// The answer a field starts out holding.
fn preset(field: &Field) -> Option<Given> {
    match &field.input {
        Input::Text { default, .. } => default.clone().map(Given::Text),
        Input::Number { default, .. } => default.map(Given::Number),
        Input::Toggle { default } => default.map(Given::Flag),
        Input::One { default, .. } => default.clone().map(Given::Chosen),
        Input::Many { default, .. } => {
            (!default.is_empty()).then(|| Given::ChosenMany(default.clone()))
        }
    }
}

/// The box a field is written in, holding the default the agent offered.
fn text_box(field: &Field) -> TextBox {
    let mut text = TextBox::one_line(&field.name);
    match &field.input {
        Input::Text {
            default: Some(default),
            ..
        } => text.set(default),
        Input::Number {
            default: Some(default),
            ..
        } => text.set(&default.to_string()),
        _ => {}
    }
    text
}

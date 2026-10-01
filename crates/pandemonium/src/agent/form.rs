//! A question the agent put to the reader, and what has been filled in so far.

use pm_acp::{Alternative, Elicitation, Field, Given, Input, Inquiry, Link, Reply};

/// An [`Elicitation`] waiting on the reader, with the answers given to it so far.
pub struct Form {
    /// What the agent asked.
    elicitation: Elicitation,
    /// The answer to each field, in the order the form lists them.
    given: Vec<Option<Given>>,
}

impl Form {
    /// A form for `elicitation`, with every field holding the default the agent offered.
    pub fn new(elicitation: Elicitation) -> Self {
        let given = match &elicitation.inquiry {
            Inquiry::Form(fields) => fields.iter().map(preset).collect(),
            Inquiry::Link(_) => Vec::new(),
        };
        Self { elicitation, given }
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

    /// What the `place`-th field is filled in with now.
    pub fn given(&self, place: usize) -> Option<&Given> {
        self.given.get(place).and_then(Option::as_ref)
    }

    /// How the `place`-th field reads in the form: its answer, or a dash.
    pub fn shown(&self, place: usize) -> String {
        let Some(field) = self.fields().get(place) else {
            return String::new();
        };
        match (self.given(place), &field.input) {
            (None, _) => "—".to_owned(),
            (Some(Given::Text(text)), _) => text.clone(),
            (Some(Given::Number(number)), _) => number.to_string(),
            (Some(Given::Flag(flag)), _) => if *flag { "Yes" } else { "No" }.to_owned(),
            (Some(Given::Chosen(value)), Input::One { options, .. }) => title_of(options, value),
            (Some(Given::ChosenMany(values)), Input::Many { options, .. }) => values
                .iter()
                .map(|value| title_of(options, value))
                .collect::<Vec<_>>()
                .join(", "),
            (Some(_), _) => "—".to_owned(),
        }
    }

    /// Fills the `place`-th field in with the line `typed`, or says why it will not take it.
    ///
    /// A blank line empties the field.
    pub fn enter(&mut self, place: usize, typed: &str) -> Result<(), String> {
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
        self.given[place] = given;
        Ok(())
    }

    /// Flips the yes-or-no field at `place`.
    pub fn flip(&mut self, place: usize) {
        let flipped = !matches!(self.given(place), Some(Given::Flag(true)));
        if matches!(
            self.fields().get(place).map(|field| &field.input),
            Some(Input::Toggle { .. })
        ) {
            self.given[place] = Some(Given::Flag(flipped));
        }
    }

    /// Chooses the `option`-th alternative of the field at `place`.
    ///
    /// A field of one choice takes it in place of the last; a field of many
    /// adds it, or takes it away where it was already chosen.
    pub fn choose(&mut self, place: usize, option: usize) {
        let Some(field) = self.fields().get(place) else {
            return;
        };
        match &field.input {
            Input::One { options, .. } => {
                if let Some(chosen) = options.get(option) {
                    self.given[place] = Some(Given::Chosen(chosen.value.clone()));
                }
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

/// What the alternative with `value` is called.
fn title_of(options: &[Alternative], value: &str) -> String {
    options
        .iter()
        .find(|option| option.value == value)
        .map_or_else(|| value.to_owned(), |option| option.title.clone())
}

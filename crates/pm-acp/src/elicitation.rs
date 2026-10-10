//! What an agent asks the reader for in the middle of its work.
//!
//! A permission is a yes or a no to one tool call. An elicitation is the
//! agent needing something the reader has and it does not: a choice among
//! options, a name, a number, or a visit to a page where the reader signs in.
//! It comes either as a form the reader fills in or as a link the reader
//! follows, and goes back as what was filled in, a refusal, or a walk away.

use serde_json::{Map, Value, json};

/// Something the agent has asked the reader for.
#[derive(Clone, Debug)]
pub struct Elicitation {
    /// The ticket the reply is given under.
    pub id: u64,
    /// What the agent says it is asking for, and why.
    pub message: String,
    /// What kind of answer it wants.
    pub inquiry: Inquiry,
}

/// The two ways an agent asks.
#[derive(Clone, Debug)]
pub enum Inquiry {
    /// Fill these in, in this order.
    Form(Vec<Field>),
    /// Go to this page; the agent says when it has what it needed.
    Link(Link),
}

/// A page the reader is asked to visit.
#[derive(Clone, Debug)]
pub struct Link {
    /// Where the page is.
    pub url: String,
    /// What the agent calls this visit when it says it is over.
    pub id: String,
}

/// One thing a form asks for.
#[derive(Clone, Debug)]
pub struct Field {
    /// The key the answer is given under.
    pub name: String,
    /// What the form calls it.
    pub title: String,
    /// What else there is to say about it.
    pub description: String,
    /// Whether the form can be sent without it.
    pub required: bool,
    /// What kind of answer it takes.
    pub input: Input,
}

/// What kind of answer a [`Field`] takes, and what it may be.
#[derive(Clone, Debug)]
pub enum Input {
    /// A line of text.
    Text {
        /// The fewest characters it may hold.
        min: Option<usize>,
        /// The most characters it may hold.
        max: Option<usize>,
        /// The answer already in it.
        default: Option<String>,
    },
    /// A number.
    Number {
        /// Whether it has to be whole.
        whole: bool,
        /// The least it may be.
        min: Option<f64>,
        /// The most it may be.
        max: Option<f64>,
        /// The answer already in it.
        default: Option<f64>,
    },
    /// Yes or no.
    Toggle {
        /// The answer already in it.
        default: Option<bool>,
    },
    /// One of these.
    One {
        /// What there is to choose from.
        options: Vec<Alternative>,
        /// The value already chosen.
        default: Option<String>,
    },
    /// Any number of these.
    Many {
        /// What there is to choose from.
        options: Vec<Alternative>,
        /// The values already chosen.
        default: Vec<String>,
        /// The fewest that may be chosen.
        min: Option<usize>,
        /// The most that may be chosen.
        max: Option<usize>,
    },
}

/// One thing a choice offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternative {
    /// What the agent is told when it is chosen.
    pub value: String,
    /// What the reader is shown.
    pub title: String,
    /// What else there is to say about it.
    pub description: String,
    /// What it would look like, where the agent drew it: a mockup, a
    /// snippet of code, two things side by side.
    pub preview: Option<String>,
}

/// What the reader filled one field in with.
#[derive(Clone, Debug, PartialEq)]
pub enum Given {
    /// A line of text.
    Text(String),
    /// A number.
    Number(f64),
    /// Yes or no.
    Flag(bool),
    /// The value of the one alternative chosen.
    Chosen(String),
    /// The values of the alternatives chosen.
    ChosenMany(Vec<String>),
}

/// What the reader answers an [`Elicitation`] with.
#[derive(Clone, Debug)]
pub enum Reply {
    /// Here is what was asked for, by field name.
    Accept(Vec<(String, Given)>),
    /// No, on purpose.
    Decline,
    /// Never mind: the reader walked away.
    Cancel,
}

impl Reply {
    /// How the protocol writes this reply down.
    pub(crate) fn wire(&self) -> Value {
        match self {
            Self::Accept(given) => {
                let content = given
                    .iter()
                    .map(|(name, given)| (name.clone(), given.wire()))
                    .collect::<Map<_, _>>();
                json!({ "action": "accept", "content": content })
            }
            Self::Decline => json!({ "action": "decline" }),
            Self::Cancel => json!({ "action": "cancel" }),
        }
    }
}

impl Given {
    /// How the protocol writes this answer down.
    fn wire(&self) -> Value {
        match self {
            Self::Text(text) => json!(text),
            Self::Number(number) if number.fract() == 0.0 && number.abs() < 9e15 => {
                json!(*number as i64)
            }
            Self::Number(number) => json!(number),
            Self::Flag(flag) => json!(flag),
            Self::Chosen(value) => json!(value),
            Self::ChosenMany(values) => json!(values),
        }
    }
}

/// The elicitation `params` make under `ticket`, or why they are not one.
pub(crate) fn read(ticket: u64, params: &Value) -> Result<Elicitation, String> {
    let message = params["message"].as_str().unwrap_or_default().to_owned();
    let inquiry = match params["mode"].as_str() {
        Some("form") => Inquiry::Form(fields(&params["requestedSchema"])),
        Some("url") => Inquiry::Link(Link {
            url: params["url"]
                .as_str()
                .filter(|url| url.starts_with("https://"))
                .ok_or("a url elicitation needs an https url")?
                .to_owned(),
            id: params["elicitationId"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        }),
        _ => return Err("an elicitation is a form or a url".to_owned()),
    };
    Ok(Elicitation {
        id: ticket,
        message,
        inquiry,
    })
}

/// The fields a schema asks for, in the order it lists them.
///
/// A property of a kind a form cannot draw is left out rather than failing
/// the whole form: the agent gets everything the reader could be asked.
fn fields(schema: &Value) -> Vec<Field> {
    let required = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    schema["properties"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, property)| {
            Some(Field {
                name: name.clone(),
                title: property["title"].as_str().unwrap_or(name).to_owned(),
                description: property["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                required: required.contains(&name.as_str()),
                input: input(property)?,
            })
        })
        .collect()
}

/// The kind of answer a property takes, if a form can ask for it.
fn input(property: &Value) -> Option<Input> {
    match property["type"].as_str()? {
        "string" => Some(match alternatives(property) {
            options if !options.is_empty() => Input::One {
                options,
                default: property["default"].as_str().map(str::to_owned),
            },
            _ => Input::Text {
                min: property["minLength"].as_u64().map(|least| least as usize),
                max: property["maxLength"].as_u64().map(|most| most as usize),
                default: property["default"].as_str().map(str::to_owned),
            },
        }),
        kind @ ("number" | "integer") => Some(Input::Number {
            whole: kind == "integer",
            min: property["minimum"].as_f64(),
            max: property["maximum"].as_f64(),
            default: property["default"].as_f64(),
        }),
        "boolean" => Some(Input::Toggle {
            default: property["default"].as_bool(),
        }),
        "array" => Some(Input::Many {
            options: alternatives(&property["items"]),
            default: property["default"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            min: property["minItems"].as_u64().map(|least| least as usize),
            max: property["maxItems"].as_u64().map(|most| most as usize),
        }),
        _ => None,
    }
}

/// What a property offers to choose from, titled where it says how.
fn alternatives(property: &Value) -> Vec<Alternative> {
    let titled = ["oneOf", "anyOf"]
        .iter()
        .find_map(|key| property[key].as_array())
        .into_iter()
        .flatten()
        .filter_map(|option| {
            let value = option["const"].as_str()?;
            Some(Alternative {
                value: value.to_owned(),
                title: option["title"].as_str().unwrap_or(value).to_owned(),
                description: option["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                preview: preview(option),
            })
        })
        .collect::<Vec<_>>();
    if !titled.is_empty() {
        return titled;
    }
    let names = property["enumNames"].as_array();
    property["enum"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(place, value)| {
            let value = value.as_str()?;
            let title = names
                .and_then(|names| names.get(place))
                .and_then(Value::as_str)
                .unwrap_or(value);
            Some(Alternative {
                value: value.to_owned(),
                title: title.to_owned(),
                description: String::new(),
                preview: None,
            })
        })
        .collect()
}

/// The preview an option carries under one of the `_meta` extensions, where
/// it carries one: the protocol has no place of its own for it yet.
fn preview(option: &Value) -> Option<String> {
    option["_meta"]
        .as_object()?
        .values()
        .find_map(|extension| extension["preview"].as_str())
        .map(str::to_owned)
}

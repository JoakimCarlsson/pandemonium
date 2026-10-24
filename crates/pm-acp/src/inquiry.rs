//! Questions the agent puts to the reader as a form: read off the agent's request, and the answers written back into the form.

use serde_json::{Map, Value, json};

/// One answer a question offers.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Alternative {
    /// What the agent calls the answer, and what it is told when the reader picks it.
    pub label: String,
    /// What picking it means, when the agent says.
    pub description: String,
}

/// One question of a form, with the answers it offers.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Question {
    /// A word or two naming what is asked, when the agent gives one.
    pub header: String,
    /// The question itself.
    pub text: String,
    /// Whether several answers may be picked.
    pub multiple: bool,
    /// The answers on offer.
    pub alternatives: Vec<Alternative>,
}

/// A form of questions the agent waits on the reader to answer before it goes on.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Inquiry {
    /// Names this form in the answer.
    pub ticket: u64,
    /// The questions, in the order they are asked.
    pub questions: Vec<Question>,
}

/// What the reader answered to one question.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reply {
    /// The labels of the alternatives picked.
    pub picks: Vec<String>,
    /// What the reader typed in place of, or beside, the alternatives.
    pub other: String,
}

/// The key of the field holding the answer to the question at `index`.
fn pick_key(index: usize) -> String {
    format!("question_{index}")
}

/// The key of the field holding what the reader typed for the question at `index`.
fn other_key(index: usize) -> String {
    format!("question_{index}_custom")
}

/// The alternatives a field of the form offers: its `oneOf` entries, or those of its items when it takes several.
fn alternatives(field: &Value) -> Vec<Alternative> {
    let entries = if field["type"] == "array" {
        &field["items"]["anyOf"]
    } else {
        &field["oneOf"]
    };
    entries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let label = entry["const"].as_str()?;
            Some(Alternative {
                label: entry["title"].as_str().unwrap_or(label).to_owned(),
                description: entry["description"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// The questions the form of an elicitation request's `params` asks, or nothing when it asks something else than a list of questions.
pub(crate) fn questions(params: &Value) -> Option<Vec<Question>> {
    if params["mode"].as_str() != Some("form") {
        return None;
    }
    let fields = params["requestedSchema"]["properties"].as_object()?;
    let single = !fields.contains_key(&pick_key(1));
    let asked: Vec<Question> = (0..)
        .map(|index| fields.get(&pick_key(index)))
        .take_while(Option::is_some)
        .flatten()
        .map(|field| {
            let alternatives = alternatives(field);
            (!alternatives.is_empty()).then(|| Question {
                header: field["title"].as_str().unwrap_or_default().to_owned(),
                text: if single {
                    params["message"].as_str().unwrap_or_default().to_owned()
                } else {
                    field["description"].as_str().unwrap_or_default().to_owned()
                },
                multiple: field["type"] == "array",
                alternatives,
            })
        })
        .collect::<Option<_>>()?;
    (!asked.is_empty() && fields.keys().all(|key| key.starts_with("question_"))).then_some(asked)
}

/// The response that hands the form `replies`, one for each of its `questions`, or declines it when there are none.
pub(crate) fn response(questions: &[Question], replies: Option<Vec<Reply>>) -> Value {
    let Some(replies) = replies else {
        return json!({ "action": "cancel" });
    };
    let mut content = Map::new();
    for (index, (question, reply)) in questions.iter().zip(replies).enumerate() {
        if !reply.picks.is_empty() {
            content.insert(
                pick_key(index),
                if question.multiple {
                    json!(reply.picks)
                } else {
                    json!(reply.picks[0])
                },
            );
        }
        if !reply.other.trim().is_empty() {
            content.insert(other_key(index), json!(reply.other.trim()));
        }
    }
    json!({ "action": "accept", "content": content })
}

/// Each of `questions` with what `replies` answered to it, joined with commas, or nothing when it was skipped.
pub(crate) fn summary(questions: &[Question], replies: &[Reply]) -> Vec<(String, Option<String>)> {
    questions
        .iter()
        .zip(replies)
        .map(|(question, reply)| {
            let mut given = reply.picks.clone();
            if !reply.other.trim().is_empty() {
                given.push(reply.other.trim().to_owned());
            }
            (
                question.text.clone(),
                (!given.is_empty()).then(|| given.join(", ")),
            )
        })
        .collect()
}

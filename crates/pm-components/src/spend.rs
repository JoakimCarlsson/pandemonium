//! What a conversation has cost, as the strip above it shows it: the total in dollars, and for each model what it cost and how many tokens it took when the pointer rests on the total.

use pm_harness::{ModelSpend, Spent};
use pm_style::{Role, cursor_default, flex, flex_none, items_center};
use pm_ui::{Node, node};

use crate::parts::line;
use crate::sample::Session;

/// How many millionths of a dollar a cent is, below which a cost is written with four decimals.
const CENT: u64 = 10_000;

/// A cost of `micros` millionths of a US dollar, in dollars: to the cent, or to a hundredth of a cent when it is less than a cent.
fn dollars(micros: u64) -> String {
    let amount = pm_harness::dollars(micros);
    if micros == 0 || micros >= CENT {
        format!("${amount:.2}")
    } else {
        format!("${amount:.4}")
    }
}

/// A count of tokens, in thousands or millions once it reaches them.
fn tokens(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => format!("{}k", count / 1_000),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}

/// A count of requests, in words.
fn requests(count: u64) -> String {
    match count {
        1 => "1 request".to_owned(),
        _ => format!("{count} requests"),
    }
}

/// What the strip says `spent` comes to: the dollars, with a `+` when a model without known prices is left out of them, or the tokens when no model's prices are known.
pub fn label(spent: &Spent) -> String {
    match spent.cost() {
        Some(cost) if spent.partial() => format!("{}+", dollars(cost)),
        Some(cost) => dollars(cost),
        None => format!("{} tokens", tokens(spent.tokens())),
    }
}

/// What `model` cost, on one line: its name, its dollars, its requests, and the tokens it was sent, with how many of them it read from the cache, and wrote.
fn model_line(model: &ModelSpend) -> String {
    let sent = model.input + model.cache_read + model.cache_write;
    format!(
        "{}: {} · {} · {} in ({} cached), {} out",
        model.model,
        model.cost.map_or_else(|| "no prices".to_owned(), dollars),
        requests(model.requests),
        tokens(sent),
        tokens(model.cache_read),
        tokens(model.output),
    )
}

/// The words that say what `spent` is made up of: the total, each model on a line of its own, and what cache misses cost when they cost something.
pub fn tooltip(spent: &Spent) -> String {
    let mut lines = vec![match spent.cost() {
        Some(cost) => format!(
            "This conversation: {} at list prices, subagents included",
            dollars(cost)
        ),
        None => "This conversation: the prices of its models are not known".to_owned(),
    }];
    lines.extend(spent.models.iter().map(model_line));
    if spent.missed() > 0 {
        lines.push(format!("Cache misses: {} of it", dollars(spent.missed())));
    }
    lines.join("\n")
}

/// What `session` has cost so far, for the strip above its conversation, with the figures as its tooltip; nothing until a model was asked.
pub fn cost_of(session: &Session) -> Option<Node> {
    let spent = session
        .spent
        .as_ref()
        .filter(|spent| spent.requests() > 0)?;
    Some(
        node()
            .styles([flex(), flex_none(), items_center(), cursor_default()])
            .child(line(label(spent), 12, 1.4, Role::Subtle))
            .tooltip(tooltip(spent)),
    )
}

/// Tests for the words a conversation's cost is shown in.
#[cfg(test)]
mod tests {
    use super::*;

    /// What a model called `name` cost: `cost` millionths of a dollar for two requests.
    fn model(name: &str, cost: Option<u64>, missed: u64) -> ModelSpend {
        ModelSpend {
            model: name.to_owned(),
            requests: 2,
            input: 1_200,
            output: 900,
            cache_read: 2_400_000,
            cache_write: 40_000,
            cost,
            missed,
        }
    }

    /// The strip shows dollars to the cent, finer below a cent, a plus when a model is left out, and tokens when no prices are known.
    #[test]
    fn the_label_is_dollars_or_tokens() {
        let priced = Spent {
            models: vec![model("opus", Some(1_237_000), 0)],
        };
        assert_eq!(label(&priced), "$1.24");
        let small = Spent {
            models: vec![model("haiku", Some(4_200), 0)],
        };
        assert_eq!(label(&small), "$0.0042");
        let partial = Spent {
            models: vec![model("opus", Some(500_000), 0), model("local", None, 0)],
        };
        assert_eq!(label(&partial), "$0.50+");
        let unpriced = Spent {
            models: vec![model("local", None, 0)],
        };
        assert_eq!(label(&unpriced), "2.4M tokens");
    }

    /// The tooltip gives the total, a line for each model and what cache misses cost.
    #[test]
    fn the_tooltip_breaks_the_cost_down() {
        let spent = Spent {
            models: vec![
                model("opus", Some(1_237_000), 310_000),
                model("local", None, 0),
            ],
        };
        assert_eq!(
            tooltip(&spent),
            "This conversation: $1.24 at list prices, subagents included\n\
             opus: $1.24 · 2 requests · 2.4M in (2.4M cached), 900 out\n\
             local: no prices · 2 requests · 2.4M in (2.4M cached), 900 out\n\
             Cache misses: $0.31 of it"
        );
    }
}

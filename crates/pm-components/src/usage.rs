//! The rings in the prompt box that show how full the conversation's context is and how much of the account's allowance is used, and the words they say when the pointer rests on them.

use pm_harness::{Limit, LimitUnit, Limits, Usage};
use pm_style::{
    Color, Role, bg, cursor_default, flex, flex_none, h_px, items_center, justify_center,
    rounded_full, w_px,
};
use pm_ui::{Node, node};

use crate::sample::Session;

/// How wide the ring of a context that has no allowance beside it is, in logical pixels.
const SINGLE_RING: f32 = 20.0;

/// How thick the ring of a context that has no allowance beside it is, in logical pixels.
const SINGLE_WIDTH: f32 = 2.5;

/// How wide the rings are together once there is more than one, in logical pixels.
const NESTED_RING: f32 = 28.0;

/// How thick each of several rings is, in logical pixels.
const NESTED_WIDTH: f32 = 2.0;

/// How much space lies between one of several rings and the next one in, in logical pixels.
const NESTED_GAP: f32 = 0.75;

/// How many rings are drawn at most; what is left over is only in the tooltip.
const MAX_RINGS: usize = 5;

/// The hue of an empty ring, in degrees: green.
const EMPTY_HUE: f32 = 145.0;

/// The hue of a full ring, in degrees: red.
const FULL_HUE: f32 = 25.0;

/// One thing the rings measure.
#[derive(Debug, PartialEq)]
struct Meter {
    /// How much of it is used, from nought to one, when that is known; a meter without a share has no ring.
    share: Option<f32>,
    /// What the tooltip says about it, on one line.
    line: String,
}

/// How full the context is, as a whole percentage, when the agent has said.
fn context_percent(context: &Usage) -> Option<u32> {
    (context.size > 0).then(|| (context.used.saturating_mul(100) / context.size).min(100) as u32)
}

/// The colour of a ring that is `used` percent full: green when empty, through amber, to red when full.
fn tone(used: u32) -> Color {
    let share = used.min(100) as f32 / 100.0;
    Color::oklch(0.72, 0.17, EMPTY_HUE + (FULL_HUE - EMPTY_HUE) * share)
}

/// A count of tokens in whole thousands once it reaches a thousand.
fn thousands(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        _ => format!("{}k", count / 1000),
    }
}

/// How long until `resets_at_ms`, from `now_ms`, as `2d 3h`, `2h 14m` or `9m`; `now` when it has passed.
fn wait(resets_at_ms: u64, now_ms: u64) -> String {
    let minutes = resets_at_ms.saturating_sub(now_ms) / 60_000;
    match (minutes / 1440, minutes % 1440 / 60, minutes % 60) {
        (0, 0, 0) => "now".to_owned(),
        (0, 0, minutes) => format!("{minutes}m"),
        (0, hours, minutes) => format!("{hours}h {minutes}m"),
        (days, hours, _) => format!("{days}d {hours}h"),
    }
}

/// `value` counted in `unit`: a dollar amount with cents, any other count without needless decimals.
fn amount(value: f64, unit: LimitUnit) -> String {
    match unit {
        LimitUnit::Usd => format!("${value:.2}"),
        _ if value.fract() == 0.0 => format!("{value:.0}"),
        _ => format!("{value:.1}"),
    }
}

/// The meter of the context, once the agent has said how full it is.
fn context_meter(context: &Usage) -> Meter {
    let used = context_percent(context);
    let tokens = format!(
        "{} of {} tokens",
        thousands(context.used),
        thousands(context.size)
    );
    Meter {
        share: used.map(|used| used as f32 / 100.0),
        line: match used {
            Some(used) => format!("Context {used}% used · {tokens}"),
            None => format!("Context {tokens}"),
        },
    }
}

/// The meter of one limit of the account, its tooltip line naming the share used, the counts and when it resets.
fn limit_meter(limit: &Limit, now_ms: u64) -> Meter {
    let mut parts = Vec::new();
    if limit.unlimited {
        parts.push("unlimited".to_owned());
    } else {
        if let Some(fraction) = limit.used_fraction {
            parts.push(format!("{:.0}% used", fraction * 100.0));
        }
        let counts = match (limit.used, limit.limit) {
            (Some(used), Some(cap)) if limit.unit != LimitUnit::Percent => Some(format!(
                "{} of {}",
                amount(used, limit.unit),
                amount(cap, limit.unit)
            )),
            (Some(used), None) if limit.unit != LimitUnit::Percent => {
                Some(format!("{} used", amount(used, limit.unit)))
            }
            _ => None,
        };
        parts.extend(counts);
    }
    if let Some(at) = limit.resets_at_ms {
        parts.push(format!("resets in {}", wait(at, now_ms)));
    }
    let line = match parts.is_empty() {
        true => limit.label.clone(),
        false => format!("{} {}", limit.label, parts.join(" · ")),
    };
    Meter {
        share: limit
            .used_fraction
            .filter(|_| !limit.unlimited)
            .map(|fraction| fraction as f32),
        line,
    }
}

/// Whether `limit` is the account-wide window named `window`, as opposed to one that covers a model or a feature, which names it in its label.
fn is_window(limit: &Limit, window: &[&str]) -> bool {
    limit
        .window
        .as_deref()
        .is_some_and(|name| window.contains(&name))
        && !limit.label.contains('(')
}

/// What the rings measure, outermost first: the context, the weekly window, the five-hour window, then every other limit of the account in the order the provider gave them; `now_ms` is the time resets are counted from.
fn meters(context: Option<&Usage>, limits: Option<&Limits>, now_ms: u64) -> Vec<Meter> {
    let mut found: Vec<Meter> = context.map(context_meter).into_iter().collect();
    let Some(limits) = limits else {
        return found;
    };
    let weekly = limits
        .limits
        .iter()
        .position(|limit| is_window(limit, &["7d", "1w"]));
    let short = limits
        .limits
        .iter()
        .position(|limit| is_window(limit, &["5h"]));
    let first: Vec<usize> = weekly.into_iter().chain(short).collect();
    let rest = (0..limits.limits.len()).filter(|index| !first.contains(index));
    for index in first.iter().copied().chain(rest) {
        found.push(limit_meter(&limits.limits[index], now_ms));
    }
    found
}

/// A few words on how much of the account's allowance is used, for a chip or a menu row: the five-hour and weekly windows when the provider has them, else the first limit it lists; nothing when it lists none.
pub fn summary(limits: &Limits) -> Option<String> {
    let used = |limit: &Limit| {
        limit
            .used_fraction
            .filter(|_| !limit.unlimited)
            .map(|fraction| (fraction * 100.0).round() as u32)
    };
    let find = |names: &[&str]| {
        limits
            .limits
            .iter()
            .find(|limit| is_window(limit, names))
            .and_then(used)
    };
    let mut parts = Vec::new();
    parts.extend(find(&["5h"]).map(|percent| format!("5h {percent}%")));
    parts.extend(find(&["7d", "1w"]).map(|percent| format!("7d {percent}%")));
    if parts.is_empty() {
        let first = limits.limits.iter().find_map(|limit| {
            used(limit).map(|percent| format!("{} {percent}%", limit.label.to_lowercase()))
        });
        parts.extend(first);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The words that say how much of the account's allowance is used, one limit to a line; nothing when the provider lists none.
pub fn tooltip(limits: &Limits) -> Option<String> {
    let lines: Vec<String> = meters(None, Some(limits), crate::age::now())
        .into_iter()
        .map(|meter| meter.line)
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The rings that show how full the context is and how much of the account's allowance is used, with the figures as their tooltip, once there is anything to show.
pub fn usage_rings(session: &Session) -> Option<Node> {
    rings_of(session.context.as_ref(), session.limits.as_ref())
}

/// The rings for `context` and `limits`, which are whatever is known so far, with the figures as their tooltip; nothing when neither is known.
pub fn rings_of(context: Option<&Usage>, limits: Option<&Limits>) -> Option<Node> {
    let found = meters(context, limits, crate::age::now());
    if found.is_empty() {
        return None;
    }
    let tooltip = found
        .iter()
        .map(|meter| meter.line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let shares: Vec<f32> = found
        .iter()
        .filter_map(|meter| meter.share)
        .take(MAX_RINGS)
        .collect();
    Some(
        node()
            .styles([
                flex(),
                flex_none(),
                items_center(),
                justify_center(),
                w_px(32),
                h_px(32),
                rounded_full(),
                cursor_default(),
                bg(Role::SurfaceHover).hover(),
            ])
            .child(rings(&shares))
            .tooltip(tooltip),
    )
}

/// The rings that fill clockwise from the top as each share grows, outermost first; one thick ring when there is one share, thinner ones set inside each other when there are several.
fn rings(shares: &[f32]) -> Node {
    let (across, width, gap) = match shares.len() {
        0 | 1 => (SINGLE_RING, SINGLE_WIDTH, 0.0),
        _ => (NESTED_RING, NESTED_WIDTH, NESTED_GAP),
    };
    node()
        .styles([flex_none(), w_px(across), h_px(across)])
        .rings(
            width,
            gap,
            Role::Border,
            shares
                .iter()
                .map(|share| (*share, tone((share * 100.0).round() as u32))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A limit with the label `label`, the window `window` and `fraction` of it used.
    fn limit(label: &str, window: &str, fraction: f64) -> Limit {
        Limit {
            window: Some(window.to_owned()),
            used_fraction: Some(fraction),
            ..Limit::new(label)
        }
    }

    /// The context comes first, then the weekly window, then the five-hour one, then the rest as the provider listed them, whatever order the provider gave the windows in.
    #[test]
    fn the_rings_are_ordered_context_weekly_short_then_the_rest() {
        let limits = Limits {
            provider: "claude".to_owned(),
            plan: None,
            fetched_at_ms: 0,
            limits: vec![
                limit("5 hour", "5h", 0.2),
                limit("7 day (Fable)", "7d", 0.9),
                limit("7 day", "7d", 0.4),
                limit("Premium requests", "monthly", 0.1),
            ],
        };
        let context = Usage {
            used: 50,
            size: 100,
            written: 0,
        };
        let found = meters(Some(&context), Some(&limits), 0);
        let shares: Vec<Option<f32>> = found.iter().map(|meter| meter.share).collect();
        assert_eq!(
            shares,
            [Some(0.5), Some(0.4), Some(0.2), Some(0.9), Some(0.1)]
        );
        assert!(found[1].line.starts_with("7 day 40% used"));
    }

    /// An unlimited quota is in the tooltip and has no ring, and a reset is told as a wait.
    #[test]
    fn unlimited_quotas_have_words_but_no_ring() {
        let mut chat = Limit::new("Chat");
        chat.unlimited = true;
        let mut week = limit("7 day", "7d", 0.25);
        week.resets_at_ms = Some(90_000_000);
        let limits = Limits {
            provider: "copilot".to_owned(),
            plan: None,
            fetched_at_ms: 0,
            limits: vec![week, chat],
        };
        let found = meters(None, Some(&limits), 0);
        assert_eq!(found[0].line, "7 day 25% used · resets in 1d 1h");
        assert_eq!(
            found[1],
            Meter {
                share: None,
                line: "Chat unlimited".to_owned()
            }
        );
    }

    /// With nothing known there is nothing to draw.
    #[test]
    fn nothing_known_draws_nothing() {
        assert!(meters(None, None, 0).is_empty());
    }

    /// The rings exist as soon as either the context or the allowance is known.
    #[test]
    fn rings_exist_once_anything_is_known() {
        let context = Usage {
            used: 50,
            size: 100,
            written: 0,
        };
        assert!(rings_of(None, None).is_none());
        assert!(rings_of(Some(&context), None).is_some());
        let limits = Limits {
            provider: "claude".to_owned(),
            plan: None,
            fetched_at_ms: 0,
            limits: vec![limit("5 hour", "5h", 0.2)],
        };
        assert!(rings_of(None, Some(&limits)).is_some());
    }
}

//! Grok's limits, as the Grok CLI answers a request of its own for them.
//!
//! Once a conversation is open the CLI answers `_x.ai/billing` with the
//! billing period, the subscription tier and how much of what the tier
//! includes is used. It goes over the agent's own pipe, so it needs no login
//! of the editor's.

use serde_json::Value;

use super::{Limits, Window, clock};

/// The method the CLI is asked under.
pub(super) const METHOD: &str = "_x.ai/billing";

/// The limits the CLI's answer `result` comes to.
///
/// An answer with no share used names no window: the tier alone is not a
/// measured limit.
pub(super) fn billing(result: &Value) -> Option<Limits> {
    let config = &result["config"];
    let period = &config["currentPeriod"];
    let window = config["creditUsagePercent"].as_f64().map(|used| Window {
        label: label(period["type"].as_str()).to_owned(),
        used,
        resets: clock::moment(&period["end"])
            .or_else(|| clock::moment(&config["billingPeriodEnd"])),
    });
    Limits::of(
        result["subscription_tier"].as_str(),
        window.into_iter().collect(),
    )
}

/// What a billing period of the type `kind` names is called.
fn label(kind: Option<&str>) -> &'static str {
    match kind {
        Some("USAGE_PERIOD_TYPE_DAILY") => "Daily",
        Some("USAGE_PERIOD_TYPE_WEEKLY") => "Weekly",
        Some("USAGE_PERIOD_TYPE_MONTHLY") => "Monthly",
        _ => "Included",
    }
}

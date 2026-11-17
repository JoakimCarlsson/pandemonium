//! What the sessions of a server have used: tokens, requests and cost, for each day, each model and each session.

use serde::{Deserialize, Serialize};

/// How much was asked of models: the requests, the tokens by kind, and what they cost.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageTotals {
    /// How many requests were answered.
    #[serde(default)]
    pub requests: u64,
    /// The input tokens that were neither read from nor written to the prompt cache.
    #[serde(default)]
    pub input: u64,
    /// The tokens written, reasoning included.
    #[serde(default)]
    pub output: u64,
    /// The input tokens read from the prompt cache.
    #[serde(default)]
    pub cache_read: u64,
    /// The input tokens written to the prompt cache.
    #[serde(default)]
    pub cache_write: u64,
    /// What it cost at list prices, in millionths of a US dollar, summed over what has known prices; nothing when none of it has.
    #[serde(default)]
    pub cost: Option<u64>,
    /// How much of `cost` was paid for cache misses, in millionths of a US dollar.
    #[serde(default)]
    pub missed: u64,
}

impl UsageTotals {
    /// Every token sent or written.
    pub fn tokens(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// Adds `other` to these totals; the cost stays unknown only while neither has one.
    pub fn add(&mut self, other: &UsageTotals) {
        self.requests += other.requests;
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.missed += other.missed;
        self.cost = match (self.cost, other.cost) {
            (None, None) => None,
            (held, added) => Some(held.unwrap_or(0) + added.unwrap_or(0)),
        };
    }

    /// Whether nothing was asked at all.
    pub fn is_empty(&self) -> bool {
        self.requests == 0 && self.tokens() == 0 && self.cost.unwrap_or(0) == 0
    }
}

/// What one model was asked.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageModel {
    /// The model's id; one served by another provider than the conversation's is named `provider/id`.
    pub model: String,
    /// How much it was asked.
    #[serde(default)]
    pub totals: UsageTotals,
}

/// What was used on one day.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageDay {
    /// The day, as `YYYY-MM-DD` in the server's time zone.
    pub day: String,
    /// What each model was asked that day, the costliest first; empty for a day nothing was asked.
    #[serde(default)]
    pub models: Vec<UsageModel>,
}

/// What one session used.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageSession {
    /// The session's number on the server.
    pub session: u64,
    /// What the session is called; empty when the server no longer holds it.
    #[serde(default)]
    pub title: String,
    /// The folder of the project it belongs to; empty for a chat.
    #[serde(default)]
    pub project: String,
    /// When it last used something in the days reported, in milliseconds since the epoch.
    #[serde(default)]
    pub updated: u64,
    /// How much it used in the days reported.
    #[serde(default)]
    pub totals: UsageTotals,
}

/// What the sessions of a server used over a run of days that ends today.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsageReport {
    /// Every day of the run, the oldest first, the ones nothing was used on included.
    #[serde(default)]
    pub days: Vec<UsageDay>,
    /// What each model was asked over the run, the costliest first.
    #[serde(default)]
    pub models: Vec<UsageModel>,
    /// The sessions that used something over the run, the costliest first; at most fifty.
    #[serde(default)]
    pub sessions: Vec<UsageSession>,
    /// Everything used over the run.
    #[serde(default)]
    pub total: UsageTotals,
}

#[cfg(test)]
/// Tests for adding usage up and writing it.
mod tests {
    use super::*;

    /// Adding totals sums every count, and the cost is known as soon as one side knows it.
    #[test]
    fn totals_add_up() {
        let mut held = UsageTotals {
            requests: 1,
            input: 10,
            output: 5,
            cache_read: 100,
            cache_write: 20,
            cost: None,
            missed: 0,
        };
        assert_eq!(held.tokens(), 135);
        held.add(&UsageTotals::default());
        assert_eq!(held.cost, None);
        held.add(&UsageTotals {
            requests: 2,
            input: 1,
            cost: Some(300),
            missed: 40,
            ..UsageTotals::default()
        });
        assert_eq!(held.requests, 3);
        assert_eq!(held.input, 11);
        assert_eq!(held.cost, Some(300));
        assert_eq!(held.missed, 40);
        assert!(!held.is_empty());
        assert!(UsageTotals::default().is_empty());
    }

    /// A report is written and read back as it was, and one written before a field existed still reads.
    #[test]
    fn a_report_round_trips() {
        let report = UsageReport {
            days: vec![UsageDay {
                day: "2026-11-17".to_owned(),
                models: vec![UsageModel {
                    model: "a".to_owned(),
                    totals: UsageTotals {
                        requests: 1,
                        cost: Some(5),
                        ..UsageTotals::default()
                    },
                }],
            }],
            ..UsageReport::default()
        };
        let text = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<UsageReport>(&text).unwrap(), report);
        let bare: UsageReport = serde_json::from_str(r#"{"days":[{"day":"2026-11-17"}]}"#).unwrap();
        assert!(bare.days[0].models.is_empty());
        assert!(bare.total.is_empty());
    }

    /// The request for a report and its answer are written under their own names and read back as they were.
    #[test]
    fn the_request_and_its_answer_round_trip() {
        let request = crate::Request::UsageReport { days: 30 };
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(text, r#"{"request":"usage_report","days":30}"#);
        assert_eq!(
            serde_json::from_str::<crate::Request>(&text).unwrap(),
            request
        );
        let answer = crate::Response::UsageReport {
            report: UsageReport::default(),
        };
        let text = serde_json::to_string(&answer).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["response"], "usage_report");
        assert_eq!(
            serde_json::from_str::<crate::Response>(&text).unwrap(),
            answer
        );
    }
}

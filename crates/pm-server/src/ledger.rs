//! The usage ledger: what every session's models were asked and what it cost, kept one line at a time with the day it happened on, and added up into the report of the last days.
//!
//! An agent reports what its conversation has cost so far as a whole. The ledger keeps the last such report of each session, works out what a new one adds to it, and writes only that, so that a report heard twice, or heard again after the server started anew, adds nothing.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{Datelike, Days, Local, NaiveDate, TimeZone};
use pm_harness::{Event, ModelSpend, Spent};
use pm_protocol::{UsageDay, UsageModel, UsageReport, UsageSession, UsageTotals};
use serde::{Deserialize, Serialize};

use crate::sessions::Meta;

/// The file under the app's data folder the ledger's lines are added to.
const LEDGER: &str = "usage.jsonl";

/// The file under the app's data folder that keeps the last report of each session, the newest line of a session winning.
const BASELINES: &str = "usage-baselines.jsonl";

/// How many sessions a report names at most.
const MOST_SESSIONS: usize = 50;

/// How many days a report covers at most.
const MOST_DAYS: u32 = 366;

/// How many lines the file of last reports may hold beyond two for each session before it is written anew with one line each.
const SLACK: usize = 64;

/// One line of the ledger: what one model was asked more, in one session, since the session's report before.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Entry {
    /// The day it was heard on, as `YYYY-MM-DD` in the server's time zone.
    pub(crate) day: String,
    /// When it was heard, in milliseconds since the epoch.
    pub(crate) at_ms: u64,
    /// The session's number.
    pub(crate) session: u64,
    /// The folder of the project the session belongs to, empty for a chat.
    #[serde(default)]
    pub(crate) project: String,
    /// The model's id.
    pub(crate) model: String,
    /// The requests added.
    #[serde(default)]
    pub(crate) requests: u64,
    /// The input tokens added that were neither read from nor written to the prompt cache.
    #[serde(default)]
    pub(crate) input: u64,
    /// The tokens written that were added.
    #[serde(default)]
    pub(crate) output: u64,
    /// The input tokens read from the prompt cache that were added.
    #[serde(default)]
    pub(crate) cache_read: u64,
    /// The input tokens written to the prompt cache that were added.
    #[serde(default)]
    pub(crate) cache_write: u64,
    /// The cost added, in millionths of a US dollar; nothing when the model's prices are not known.
    #[serde(default)]
    pub(crate) cost: Option<u64>,
    /// How much of the cost added was paid for cache misses, in millionths of a US dollar.
    #[serde(default)]
    pub(crate) missed: u64,
}

impl Entry {
    /// What the line adds, as the report counts it.
    fn totals(&self) -> UsageTotals {
        UsageTotals {
            requests: self.requests,
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            cost: self.cost,
            missed: self.missed,
        }
    }
}

/// One line of the file of last reports: a session and what its agent last said its conversation cost.
#[derive(Serialize, Deserialize)]
struct Baseline {
    /// The session's number.
    session: u64,
    /// What its conversation had cost when last heard.
    spent: Spent,
}

/// What a session is called and the project it belongs to, for the report to name it by.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Known {
    /// What the session is called.
    pub(crate) title: String,
    /// The folder of the project it belongs to, empty for a chat.
    pub(crate) project: String,
}

/// The day the moment `at_ms` milliseconds since the epoch falls on in the server's time zone, as `YYYY-MM-DD`.
pub(crate) fn day_of(at_ms: u64) -> String {
    let moment = i64::try_from(at_ms)
        .ok()
        .and_then(|at| Local.timestamp_millis_opt(at).earliest());
    match moment {
        Some(moment) => written(moment.date_naive()),
        None => written(NaiveDate::default()),
    }
}

/// `date` as `YYYY-MM-DD`.
fn written(date: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

/// Whether `current` holds less of something than `previous` did, or lost a model: the conversation was reset, rewound or branched, so `current` is no continuation of `previous`.
fn went_down(previous: &Spent, current: &Spent) -> bool {
    previous.models.iter().any(|before| {
        current
            .models
            .iter()
            .find(|now| now.model == before.model)
            .is_none_or(|now| {
                now.requests < before.requests
                    || now.input < before.input
                    || now.output < before.output
                    || now.cache_read < before.cache_read
                    || now.cache_write < before.cache_write
            })
    })
}

/// What `current` adds to `previous` for each model, leaving out the models nothing was added to; all of `current` when there is no `previous`, and nothing when `current` went down from it, since it then only says where to count on from.
pub(crate) fn added(previous: Option<&Spent>, current: &Spent) -> Vec<ModelSpend> {
    if previous.is_some_and(|previous| went_down(previous, current)) {
        return Vec::new();
    }
    let nothing = ModelSpend::default();
    current
        .models
        .iter()
        .map(|now| {
            let before = previous
                .and_then(|previous| previous.models.iter().find(|held| held.model == now.model))
                .unwrap_or(&nothing);
            ModelSpend {
                model: now.model.clone(),
                requests: now.requests - before.requests,
                input: now.input - before.input,
                output: now.output - before.output,
                cache_read: now.cache_read - before.cache_read,
                cache_write: now.cache_write - before.cache_write,
                cost: now
                    .cost
                    .map(|cost| cost.saturating_sub(before.cost.unwrap_or(0))),
                missed: now.missed.saturating_sub(before.missed),
            }
        })
        .filter(|more| {
            more.requests > 0 || more.tokens() > 0 || more.cost.unwrap_or(0) > 0 || more.missed > 0
        })
        .collect()
}

/// The ledger of one data folder: its files, and the last report of each session.
pub(crate) struct Ledger {
    /// The folder the ledger's files are in.
    folder: PathBuf,
    /// What each session's conversation had cost when last heard, by the session's number.
    baselines: HashMap<u64, Spent>,
}

impl Ledger {
    /// The ledger kept in `folder`, with the last reports read from it; the file of last reports is written anew, one line for each session, when it has grown long.
    pub(crate) fn open(folder: PathBuf) -> Self {
        let text = std::fs::read_to_string(folder.join(BASELINES)).unwrap_or_default();
        let mut lines = 0;
        let mut baselines = HashMap::new();
        for line in text.lines() {
            if let Ok(held) = serde_json::from_str::<Baseline>(line) {
                lines += 1;
                baselines.insert(held.session, held.spent);
            }
        }
        let ledger = Self { folder, baselines };
        if lines > 2 * ledger.baselines.len() + SLACK {
            ledger.rewrite_baselines();
        }
        ledger
    }

    /// Writes the file of last reports anew with one line for each session.
    fn rewrite_baselines(&self) {
        let mut numbers: Vec<&u64> = self.baselines.keys().collect();
        numbers.sort();
        let text: String = numbers
            .into_iter()
            .filter_map(|session| {
                serde_json::to_string(&Baseline {
                    session: *session,
                    spent: self.baselines[session].clone(),
                })
                .ok()
            })
            .map(|line| line + "\n")
            .collect();
        if let Ok(mut file) = pm_store::open_private(&self.folder.join(BASELINES), false) {
            let _ = file.write_all(text.as_bytes());
        }
    }

    /// Adds `lines`, one to a line, to the end of the file called `name` in the ledger's folder.
    fn append(&self, name: &str, lines: &[String]) {
        if lines.is_empty() || pm_store::create_private_directory(&self.folder).is_err() {
            return;
        }
        if let Ok(mut file) = pm_store::open_private(&self.folder.join(name), true) {
            let mut text = lines.join("\n");
            text.push('\n');
            let _ = file.write_all(text.as_bytes());
        }
    }

    /// Whether a report of the session numbered `session` was ever kept.
    pub(crate) fn knows(&self, session: u64) -> bool {
        self.baselines.contains_key(&session)
    }

    /// Keeps `spent` as the last report of the session numbered `session`, in memory and on disk.
    fn keep(&mut self, session: u64, spent: &Spent) {
        let line = serde_json::to_string(&Baseline {
            session,
            spent: spent.clone(),
        });
        self.append(BASELINES, &line.into_iter().collect::<Vec<_>>());
        self.baselines.insert(session, spent.clone());
    }

    /// Takes in `spent`, what the conversation of the session numbered `session`, of the project `project`, has cost so far as heard at `at_ms`: writes a line for each model it asked more since the session's last report, keeps `spent` as that report, and returns the lines written. A report equal to the last writes nothing, and one that went down only becomes the report to count on from.
    pub(crate) fn record(
        &mut self,
        session: u64,
        project: &str,
        at_ms: u64,
        spent: &Spent,
    ) -> Vec<Entry> {
        let previous = self.baselines.get(&session);
        if previous == Some(spent) {
            return Vec::new();
        }
        let day = day_of(at_ms);
        let entries: Vec<Entry> = added(previous, spent)
            .into_iter()
            .map(|more| Entry {
                day: day.clone(),
                at_ms,
                session,
                project: project.to_owned(),
                model: more.model,
                requests: more.requests,
                input: more.input,
                output: more.output,
                cache_read: more.cache_read,
                cache_write: more.cache_write,
                cost: more.cost,
                missed: more.missed,
            })
            .collect();
        let lines: Vec<String> = entries
            .iter()
            .filter_map(|entry| serde_json::to_string(entry).ok())
            .collect();
        self.append(LEDGER, &lines);
        self.keep(session, spent);
        entries
    }

    /// Has the session numbered `to` count on from the last report of the session numbered `from`, whose conversation it starts with a copy of, so that what the one already paid is not entered again for the other; nothing happens when `from` has no report.
    pub(crate) fn inherit(&mut self, from: u64, to: u64) {
        if let Some(spent) = self.baselines.get(&from).cloned() {
            self.keep(to, &spent);
        }
    }

    /// Every line of the ledger, in the order they were written.
    pub(crate) fn entries(&self) -> Vec<Entry> {
        read_entries(&self.folder)
    }
}

/// Every line of the ledger kept in `folder`, in the order they were written; a line that cannot be read is left out.
fn read_entries(folder: &Path) -> Vec<Entry> {
    std::fs::read_to_string(folder.join(LEDGER))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Adds `totals` to what `models` holds for the model called `model`, which is added to the list when it is not in it.
fn add_to(models: &mut Vec<UsageModel>, model: &str, totals: &UsageTotals) {
    match models.iter_mut().find(|held| held.model == model) {
        Some(held) => held.totals.add(totals),
        None => models.push(UsageModel {
            model: model.to_owned(),
            totals: totals.clone(),
        }),
    }
}

/// What `totals` is sorted by so that the costliest comes first: its cost, then its tokens, both turned around.
fn weight(totals: &UsageTotals) -> (std::cmp::Reverse<u64>, std::cmp::Reverse<u64>) {
    (
        std::cmp::Reverse(totals.cost.unwrap_or(0)),
        std::cmp::Reverse(totals.tokens()),
    )
}

/// The report of the `days` days that end on `today`, added up from `entries`, with the sessions named as `known` says, by number. `days` is at least one and at most a year.
pub(crate) fn report(
    entries: &[Entry],
    known: &HashMap<u64, Known>,
    today: NaiveDate,
    days: u32,
) -> UsageReport {
    let days = days.clamp(1, MOST_DAYS);
    let mut report = UsageReport {
        days: (0..days)
            .rev()
            .filter_map(|back| today.checked_sub_days(Days::new(u64::from(back))))
            .map(|date| UsageDay {
                day: written(date),
                models: Vec::new(),
            })
            .collect(),
        ..UsageReport::default()
    };
    let mut sessions: Vec<UsageSession> = Vec::new();
    for entry in entries {
        let Some(day) = report.days.iter_mut().find(|day| day.day == entry.day) else {
            continue;
        };
        let totals = entry.totals();
        add_to(&mut day.models, &entry.model, &totals);
        add_to(&mut report.models, &entry.model, &totals);
        report.total.add(&totals);
        let place = sessions
            .iter()
            .position(|held| held.session == entry.session)
            .unwrap_or_else(|| {
                let named = known.get(&entry.session);
                sessions.push(UsageSession {
                    session: entry.session,
                    title: named.map(|named| named.title.clone()).unwrap_or_default(),
                    project: named
                        .map_or_else(|| entry.project.clone(), |named| named.project.clone()),
                    updated: 0,
                    totals: UsageTotals::default(),
                });
                sessions.len() - 1
            });
        sessions[place].totals.add(&totals);
        sessions[place].updated = sessions[place].updated.max(entry.at_ms);
    }
    for day in &mut report.days {
        day.models.sort_by_key(|model| weight(&model.totals));
    }
    report.models.sort_by_key(|model| weight(&model.totals));
    sessions.sort_by_key(|session| (weight(&session.totals), std::cmp::Reverse(session.updated)));
    sessions.truncate(MOST_SESSIONS);
    report.sessions = sessions;
    report
}

/// The ledger of the app's data folder, once something asked for it.
static SHARED: Mutex<Option<Ledger>> = Mutex::new(None);

/// Does `work` with the ledger of the app's data folder, opening it first when nothing has yet, and returns what it gave; nothing in a test, which must not write into the reader's ledger, or when the desktop has no data folder.
fn with_shared<T>(work: impl FnOnce(&mut Ledger) -> T) -> Option<T> {
    if cfg!(test) {
        return None;
    }
    let folder = pm_store::data_directory()?;
    let mut held = SHARED.lock().ok()?;
    Some(work(held.get_or_insert_with(|| Ledger::open(folder))))
}

/// Enters what `spent`, which the agent of the session numbered `session` just reported, adds to the session's last report, dated now.
pub(crate) fn heard(session: u64, meta: &Meta, spent: &Spent) {
    with_shared(|ledger| ledger.record(session, &meta.repository, crate::core::now(), spent));
}

/// Enters the session numbered `session`, read from disk with `log`, when the ledger has never heard of it: what its saved conversation cost, dated when the session was last worked in, so that sessions older than the ledger are counted once and not again when their agents report the same total.
pub(crate) fn backfill(session: u64, meta: &Meta, log: &[(u64, Event)]) {
    let saved = log.iter().rev().find_map(|(_, event)| match event {
        Event::Spent(spent) => Some(spent),
        _ => None,
    });
    if let Some(spent) = saved {
        with_shared(|ledger| {
            if !ledger.knows(session) {
                ledger.record(session, &meta.repository, meta.updated, spent);
            }
        });
    }
}

/// Has the session numbered `to` count on from the last report of the session numbered `from`, whose saved conversation it was given a copy of.
pub(crate) fn inherit(from: u64, to: u64) {
    with_shared(|ledger| ledger.inherit(from, to));
}

/// The report of the last `days` days, today included, with the sessions named as `known` says; its days with nothing in them when there is no ledger.
fn usage(days: u32, known: &HashMap<u64, Known>) -> UsageReport {
    let today = Local::now().date_naive();
    let entries = with_shared(|ledger| ledger.entries()).unwrap_or_default();
    report(&entries, known, today, days)
}

/// The report of the last `days` days, today included, of the sessions `core` holds and of the ones it once held, each named as `core` knows it now.
pub(crate) fn asked(core: &crate::core::Core, days: u32) -> UsageReport {
    let known: HashMap<u64, Known> = core
        .sessions
        .lock()
        .map(|sessions| {
            sessions
                .iter()
                .map(|(number, session)| {
                    (
                        *number,
                        Known {
                            title: session.meta.title.clone(),
                            project: session.meta.repository.clone(),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    usage(days, &known)
}

#[cfg(test)]
/// Tests for what a report adds to the one before it, for keeping it across a restart, and for adding the ledger up.
mod tests {
    use super::*;

    /// What one model cost: `requests` requests of ten input and five output tokens each, a hundred read from the cache, at a thousand millionths of a dollar a request when `priced`.
    fn model(name: &str, requests: u64, priced: bool) -> ModelSpend {
        ModelSpend {
            model: name.to_owned(),
            requests,
            input: requests * 10,
            output: requests * 5,
            cache_read: requests * 100,
            cache_write: 0,
            cost: priced.then_some(requests * 1_000),
            missed: 0,
        }
    }

    /// A report of the models `models`.
    fn spent(models: Vec<ModelSpend>) -> Spent {
        Spent { models }
    }

    /// An empty scratch folder for the test called `name`, under the build folder.
    fn scratch(name: &str) -> PathBuf {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/ledger-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    /// The first report of a session is entered whole.
    #[test]
    fn the_first_report_is_entered_whole() {
        let first = spent(vec![model("a", 2, true), model("b", 1, false)]);
        assert_eq!(added(None, &first), first.models);
    }

    /// A report that grew adds only the difference, for the models that grew and the ones that are new.
    #[test]
    fn growth_adds_the_difference() {
        let before = spent(vec![model("a", 2, true), model("b", 1, false)]);
        let after = spent(vec![
            model("a", 5, true),
            model("b", 1, false),
            model("c", 1, true),
        ]);
        assert_eq!(
            added(Some(&before), &after),
            [model("a", 3, true), model("c", 1, true)]
        );
    }

    /// A model whose prices became known is entered with the cost it then has.
    #[test]
    fn a_cost_that_appears_is_entered() {
        let before = spent(vec![model("a", 2, false)]);
        let after = spent(vec![model("a", 3, true)]);
        let more = added(Some(&before), &after);
        assert_eq!(more[0].requests, 1);
        assert_eq!(more[0].cost, Some(3_000));
    }

    /// The same report again adds nothing.
    #[test]
    fn a_replayed_report_adds_nothing() {
        let held = spent(vec![model("a", 2, true)]);
        assert!(added(Some(&held), &held).is_empty());
    }

    /// A report lower than the last, or without one of its models, adds nothing: nothing is entered below zero.
    #[test]
    fn a_reset_adds_nothing() {
        let before = spent(vec![model("a", 5, true), model("b", 1, true)]);
        assert!(
            added(
                Some(&before),
                &spent(vec![model("a", 2, true), model("b", 1, true)])
            )
            .is_empty()
        );
        assert!(added(Some(&before), &spent(vec![model("a", 9, true)])).is_empty());
        assert!(added(Some(&before), &Spent::default()).is_empty());
    }

    /// Recording writes the lines and keeps the report, a replay writes nothing, a reset writes nothing and growth after it counts from the reset.
    #[test]
    fn recording_follows_a_session_through_a_reset() {
        let folder = scratch("reset");
        let mut ledger = Ledger::open(folder.clone());
        let first = ledger.record(7, "/code/app", 1_000, &spent(vec![model("a", 2, true)]));
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].session, 7);
        assert_eq!(first[0].project, "/code/app");
        assert_eq!(first[0].day, day_of(1_000));
        assert_eq!(first[0].cost, Some(2_000));
        assert!(
            ledger
                .record(7, "/code/app", 2_000, &spent(vec![model("a", 2, true)]))
                .is_empty()
        );
        assert!(
            ledger
                .record(7, "/code/app", 3_000, &spent(vec![model("a", 1, true)]))
                .is_empty()
        );
        let grown = ledger.record(7, "/code/app", 4_000, &spent(vec![model("a", 4, true)]));
        assert_eq!(grown.len(), 1);
        assert_eq!(grown[0].requests, 3);
        let requests: u64 = ledger.entries().iter().map(|entry| entry.requests).sum();
        assert_eq!(requests, 5);
        let _ = std::fs::remove_dir_all(folder);
    }

    /// A ledger opened again on the same folder remembers each session's last report, so the same report adds nothing and a grown one adds the difference.
    #[test]
    fn a_restart_adds_nothing_twice() {
        let folder = scratch("restart");
        let mut ledger = Ledger::open(folder.clone());
        ledger.record(3, "", 1_000, &spent(vec![model("a", 2, true)]));
        ledger.record(4, "", 1_000, &spent(vec![model("b", 1, false)]));
        ledger.record(3, "", 2_000, &spent(vec![model("a", 3, true)]));
        let mut again = Ledger::open(folder.clone());
        assert!(again.knows(3) && again.knows(4) && !again.knows(5));
        assert!(
            again
                .record(3, "", 5_000, &spent(vec![model("a", 3, true)]))
                .is_empty()
        );
        let grown = again.record(3, "", 6_000, &spent(vec![model("a", 4, true)]));
        assert_eq!(grown.len(), 1);
        assert_eq!(grown[0].requests, 1);
        assert_eq!(again.entries().len(), 4);
        let _ = std::fs::remove_dir_all(folder);
    }

    /// A session given a copy of another's conversation counts on from the other's report.
    #[test]
    fn a_branch_counts_on_from_its_source() {
        let folder = scratch("branch");
        let mut ledger = Ledger::open(folder.clone());
        ledger.record(1, "", 1_000, &spent(vec![model("a", 4, true)]));
        ledger.inherit(1, 2);
        ledger.inherit(9, 10);
        assert!(!ledger.knows(10));
        assert!(
            ledger
                .record(2, "", 2_000, &spent(vec![model("a", 4, true)]))
                .is_empty()
        );
        let grown = ledger.record(2, "", 3_000, &spent(vec![model("a", 6, true)]));
        assert_eq!(grown[0].requests, 2);
        let _ = std::fs::remove_dir_all(folder);
    }

    /// The file of last reports is written anew with one line for each session once it has grown long, and still reads the same.
    #[test]
    fn the_last_reports_are_compacted() {
        let folder = scratch("compact");
        let mut ledger = Ledger::open(folder.clone());
        for requests in 1..=(SLACK as u64 + 10) {
            ledger.record(1, "", requests, &spent(vec![model("a", requests, true)]));
        }
        let lines = |folder: &Path| {
            std::fs::read_to_string(folder.join(BASELINES))
                .unwrap()
                .lines()
                .count()
        };
        assert_eq!(lines(&folder), SLACK + 10);
        let again = Ledger::open(folder.clone());
        assert_eq!(lines(&folder), 1);
        assert_eq!(
            again.baselines[&1],
            spent(vec![model("a", SLACK as u64 + 10, true)])
        );
        let _ = std::fs::remove_dir_all(folder);
    }

    /// A line of the ledger for the model `name` of the session `session`, on `day`, of `requests` requests.
    fn entry(day: &str, session: u64, name: &str, requests: u64, priced: bool) -> Entry {
        let more = model(name, requests, priced);
        Entry {
            day: day.to_owned(),
            at_ms: session * 100 + requests,
            session,
            project: "/code/old".to_owned(),
            model: more.model,
            requests: more.requests,
            input: more.input,
            output: more.output,
            cache_read: more.cache_read,
            cache_write: more.cache_write,
            cost: more.cost,
            missed: more.missed,
        }
    }

    /// The report holds every day of the run, leaves out what is older or newer, and adds the rest up by day, by model and by session, the costliest first.
    #[test]
    fn the_report_adds_the_days_up() {
        let entries = [
            entry("2026-11-10", 1, "a", 9, true),
            entry("2026-11-15", 1, "a", 2, true),
            entry("2026-11-15", 2, "b", 5, true),
            entry("2026-11-17", 1, "a", 1, true),
            entry("2026-11-17", 3, "c", 4, false),
            entry("2026-11-18", 1, "a", 7, true),
        ];
        let known = HashMap::from([(
            1,
            Known {
                title: "Fix the parser".to_owned(),
                project: "/code/app".to_owned(),
            },
        )]);
        let today = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let report = report(&entries, &known, today, 3);
        let days: Vec<&str> = report.days.iter().map(|day| day.day.as_str()).collect();
        assert_eq!(days, ["2026-11-15", "2026-11-16", "2026-11-17"]);
        assert_eq!(report.days[0].models[0].model, "b");
        assert_eq!(report.days[0].models[1].model, "a");
        assert!(report.days[1].models.is_empty());
        let models: Vec<&str> = report
            .models
            .iter()
            .map(|model| model.model.as_str())
            .collect();
        assert_eq!(models, ["b", "a", "c"]);
        assert_eq!(report.models[1].totals.requests, 3);
        assert_eq!(report.models[2].totals.cost, None);
        assert_eq!(report.total.requests, 12);
        assert_eq!(report.total.cost, Some(8_000));
        assert_eq!(report.total.cache_read, 1_200);
        let sessions: Vec<u64> = report
            .sessions
            .iter()
            .map(|session| session.session)
            .collect();
        assert_eq!(sessions, [2, 1, 3]);
        assert_eq!(report.sessions[1].title, "Fix the parser");
        assert_eq!(report.sessions[1].project, "/code/app");
        assert_eq!(report.sessions[1].updated, 102);
        assert_eq!(report.sessions[0].title, "");
        assert_eq!(report.sessions[0].project, "/code/old");
    }

    /// A report of no days covers today, and one of nothing has its days and nothing else.
    #[test]
    fn an_empty_ledger_reports_empty_days() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let report = report(&[], &HashMap::new(), today, 0);
        assert_eq!(report.days.len(), 1);
        assert_eq!(report.days[0].day, "2026-03-01");
        assert!(report.total.is_empty() && report.sessions.is_empty());
        let month = super::report(&[], &HashMap::new(), today, 30);
        assert_eq!(month.days[0].day, "2026-01-31");
    }
}

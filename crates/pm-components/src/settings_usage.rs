//! The usage page of the settings window: what the sessions of every connected server used over the last days, as headline totals, a bar for each day stacked by model, a row for each model and the sessions that used most.

use pm_input::FocusId;
use pm_protocol::{UsageDay, UsageModel, UsageReport, UsageSession, UsageTotals};
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, flex, flex_col, flex_none, flex_wrap,
    gap, h_px, items_center, items_end, justify_between, justify_end, p, px, rounded, w_px,
};
use pm_ui::{Node, node};

use crate::BarRow;
use crate::parts::line;
use crate::settings_ids::USAGE_RANGES;
use crate::settings_row::choices;
use crate::settings_servers::{boxed, heading_only, note, paragraph, wrapped_note};
use crate::spend::{dollars, requests, tokens};
use crate::state::{Outgoing, WorkspaceState};

/// What the page says about itself, for the search and the overview.
pub(crate) const SUMMARY: &str = "What your sessions asked of models: tokens, requests and spend at list prices, for each day, each model and each session, on every server the window is connected to.";

/// The runs of days the page can cover, today included, in the order they are offered.
pub const RANGES: [u32; 3] = [7, 30, 90];

/// What the choices of [`RANGES`] are called.
const RANGE_NAMES: [&str; 3] = ["7 days", "30 days", "90 days"];

/// How tall the tallest bar of the chart is, in logical pixels.
const CHART_HEIGHT: f32 = 96.0;

/// How wide a bar of the chart is at most, in logical pixels.
const WIDEST_BAR: f32 = 36.0;

/// How many models the chart gives a colour of their own; the rest share one.
const MOST_SERIES: usize = 5;

/// How many sessions the page lists.
const TOP_SESSIONS: usize = 5;

/// The hues of the colours the chart gives its models, in the order of the models.
const SERIES_HUES: [f32; MOST_SERIES] = [255.0, 150.0, 70.0, 320.0, 25.0];

/// How wide the page is at least for its four headline figures to stand side by side, in logical pixels.
const WIDE_HEADLINE: f32 = 520.0;

/// What the models past the ones with a colour of their own are called together.
const OTHER: &str = "Other";

/// What one server reported.
#[derive(Clone, Debug, PartialEq)]
pub struct HostReport {
    /// The machine the server is on, or nothing for this one.
    pub host: Option<String>,
    /// What its sessions used.
    pub report: UsageReport,
}

/// What the usage page holds: the days it covers, what each server reported, and why one could not.
#[derive(Clone, Debug, PartialEq)]
pub struct UsagePage {
    /// How many days the page covers, today included.
    pub days: u32,
    /// Whether the servers were asked and none has answered yet.
    pub loading: bool,
    /// What each server reported for `days`.
    pub reports: Vec<HostReport>,
    /// The servers that could not report, each with why.
    pub errors: Vec<(Option<String>, String)>,
}

impl Default for UsagePage {
    /// A page covering thirty days, with nothing asked yet.
    fn default() -> Self {
        Self {
            days: RANGES[1],
            loading: false,
            reports: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// What the bars measure: spend when something has known prices, tokens otherwise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Measure {
    /// Millionths of a US dollar at list prices.
    Cost,
    /// Tokens sent and written.
    Tokens,
}

impl WorkspaceState {
    /// Asks every connected server what its sessions used over the days the page covers.
    pub(crate) fn ask_usage(&mut self) {
        let page = &mut self.settings_page.usage;
        page.loading = true;
        page.errors.clear();
        let days = page.days;
        self.outbox.push(Outgoing::UsageReport { days });
    }

    /// Takes the press of `action` when it names a choice of how many days the usage page covers: covers that many, drops the reports of another run of days and asks again. Returns whether it was one.
    pub(crate) fn usage_pressed(&mut self, action: FocusId) -> bool {
        let Some(days) = USAGE_RANGES
            .index_of(action)
            .and_then(|option| RANGES.get(option).copied())
        else {
            return false;
        };
        if self.settings_page.usage.days != days {
            self.settings_page.usage.days = days;
            self.settings_page.usage.reports.clear();
        }
        self.ask_usage();
        true
    }

    /// The state with what the server on `host` answered when asked for the usage of `days` days: its report, or why it has none. An answer about another run of days than the page covers is dropped.
    pub fn with_usage_report(
        mut self,
        host: Option<String>,
        days: u32,
        answer: Result<UsageReport, String>,
    ) -> Self {
        let page = &mut self.settings_page.usage;
        if page.days != days {
            return self;
        }
        page.loading = false;
        page.reports.retain(|held| held.host != host);
        page.errors.retain(|(held, _)| *held != host);
        match answer {
            Ok(report) => page.reports.push(HostReport { host, report }),
            Err(reason) => page.errors.push((host, reason)),
        }
        self
    }
}

/// Adds `totals` to what `models` holds for the model called `model`, which is added to the list when it is not in it.
fn add_model(models: &mut Vec<UsageModel>, model: &str, totals: &UsageTotals) {
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

/// The reports of every server added up into one: each day's models summed by day, the models summed by name, the sessions of all of them side by side, the costliest first.
pub(crate) fn merged(reports: &[HostReport]) -> UsageReport {
    let mut whole = UsageReport::default();
    for held in reports {
        let report = &held.report;
        for day in &report.days {
            let place = whole
                .days
                .iter()
                .position(|known| known.day == day.day)
                .unwrap_or_else(|| {
                    whole.days.push(UsageDay {
                        day: day.day.clone(),
                        models: Vec::new(),
                    });
                    whole.days.len() - 1
                });
            for model in &day.models {
                add_model(&mut whole.days[place].models, &model.model, &model.totals);
            }
        }
        for model in &report.models {
            add_model(&mut whole.models, &model.model, &model.totals);
        }
        whole.sessions.extend(report.sessions.iter().cloned());
        whole.total.add(&report.total);
    }
    whole.days.sort_by(|one, other| one.day.cmp(&other.day));
    for day in &mut whole.days {
        day.models.sort_by_key(|model| weight(&model.totals));
    }
    whole.models.sort_by_key(|model| weight(&model.totals));
    whole
        .sessions
        .sort_by_key(|session| weight(&session.totals));
    whole
}

/// What the bars of `report` measure: spend when anything in it has a cost, tokens otherwise.
pub(crate) fn measure(report: &UsageReport) -> Measure {
    if report.total.cost.is_some_and(|cost| cost > 0) {
        Measure::Cost
    } else {
        Measure::Tokens
    }
}

/// How much `totals` comes to in `measure`.
pub(crate) fn amount(totals: &UsageTotals, measure: Measure) -> u64 {
    match measure {
        Measure::Cost => totals.cost.unwrap_or(0),
        Measure::Tokens => totals.tokens(),
    }
}

/// `amount` of `measure` in words: dollars, or a count of tokens.
pub(crate) fn figure(amount: u64, measure: Measure) -> String {
    match measure {
        Measure::Cost => dollars(amount),
        Measure::Tokens => format!("{} tokens", tokens(amount)),
    }
}

/// What `totals` comes to in words: its dollars, or its tokens when nothing in it has known prices.
pub(crate) fn spend_label(totals: &UsageTotals) -> String {
    match totals.cost {
        Some(cost) => dollars(cost),
        None => format!("{} tokens", tokens(totals.tokens())),
    }
}

/// How much of the input `totals` sent was read from the prompt cache, as a whole percentage; nothing when no input was sent.
pub(crate) fn cache_share(totals: &UsageTotals) -> Option<u32> {
    let sent = totals.input + totals.cache_read + totals.cache_write;
    (sent > 0).then(|| (totals.cache_read.saturating_mul(100) / sent) as u32)
}

/// The day written `YYYY-MM-DD` as a short date such as `Nov 17`; the day as written when it is not one.
pub(crate) fn day_label(day: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = day.split('-').skip(1);
    let month = parts
        .next()
        .and_then(|month| month.parse::<usize>().ok())
        .and_then(|month| MONTHS.get(month.wrapping_sub(1)));
    let date = parts.next().and_then(|date| date.parse::<u32>().ok());
    match (month, date) {
        (Some(month), Some(date)) => format!("{month} {date}"),
        _ => day.to_owned(),
    }
}

/// What the project in the folder `project` is called: the folder's own name, or `Chat` for a session that belongs to none.
pub(crate) fn project_name(project: &str) -> String {
    let name = project
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    if name.is_empty() {
        "Chat".to_owned()
    } else {
        name.to_owned()
    }
}

/// What the session `session` is called on the page: its title, or its number when the server no longer holds it.
pub(crate) fn session_name(session: &UsageSession) -> String {
    if session.title.trim().is_empty() {
        format!("Session {}", session.session)
    } else {
        session.title.clone()
    }
}

/// The models of `report` the chart gives a colour of their own, the costliest first, at most [`MOST_SERIES`].
pub(crate) fn series(report: &UsageReport) -> Vec<String> {
    report
        .models
        .iter()
        .take(MOST_SERIES)
        .map(|model| model.model.clone())
        .collect()
}

/// How much `day` comes to in `measure` for each of `series`, in their order, and after them for every other model together.
pub(crate) fn stack(day: &UsageDay, series: &[String], measure: Measure) -> Vec<u64> {
    let mut amounts = vec![0; series.len() + 1];
    for model in &day.models {
        let place = series
            .iter()
            .position(|name| *name == model.model)
            .unwrap_or(series.len());
        amounts[place] += amount(&model.totals, measure);
    }
    amounts
}

/// How tall each part of a bar holding `amounts` is, in logical pixels, when a bar of `most` is `height` tall.
pub(crate) fn heights(amounts: &[u64], most: u64, height: f32) -> Vec<f32> {
    amounts
        .iter()
        .map(|amount| {
            if most == 0 {
                0.0
            } else {
                *amount as f32 * height / most as f32
            }
        })
        .collect()
}

/// How wide each of `count` bars is, in logical pixels, for them to stand side by side with a gap in `room` pixels: at least one and at most [`WIDEST_BAR`].
pub(crate) fn bar_width(room: f32, count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let gap = if count > 31 { 1.0 } else { 2.0 };
    let free = room - gap * (count - 1) as f32;
    (free / count as f32).floor().clamp(1.0, WIDEST_BAR)
}

/// The colour of the part of a bar at `place` among the chart's models; the one every other model shares past the last of them.
fn series_tone(place: usize) -> Color {
    match SERIES_HUES.get(place) {
        Some(hue) => Color::oklch(0.72, 0.15, *hue),
        None => Role::Subtle.into(),
    }
}

/// One headline figure, `width` logical pixels across: what it is, how much, and a quiet line saying more.
fn headline(title: &str, value: String, more: String, width: f32) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(0.5),
            p(3),
            w_px(width),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line(title, 12, 1.4, Role::Muted))
        .child(line(value, 18, 1.3, Role::Text).styles([pm_style::font_medium()]))
        .child(line(more, 12, 1.4, Role::Muted))
}

/// The headline figures of `report` in `room` logical pixels: spend, tokens, requests and how much of the input was read from the cache, four side by side or two when the page is narrow.
fn headlines(report: &UsageReport, room: f32) -> Node {
    let total = &report.total;
    let columns = if room >= WIDE_HEADLINE { 4.0 } else { 2.0 };
    let width = ((room - 8.0 * (columns - 1.0)) / columns).floor() - 4.0;
    let sent = total.input + total.cache_read + total.cache_write;
    let spend = match total.cost {
        Some(cost) => (
            dollars(cost),
            if total.missed > 0 {
                format!("{} on cache misses", dollars(total.missed))
            } else {
                "at list prices".to_owned()
            },
        ),
        None => ("—".to_owned(), "no known prices".to_owned()),
    };
    let sessions = match report.sessions.len() {
        1 => "in 1 session".to_owned(),
        count => format!("in {count} sessions"),
    };
    node()
        .styles([flex(), flex_wrap(), flex_none(), gap(2)])
        .child(headline("Spend", spend.0, spend.1, width))
        .child(headline(
            "Tokens",
            tokens(total.tokens()),
            format!("{} in · {} out", tokens(sent), tokens(total.output)),
            width,
        ))
        .child(headline(
            "Requests",
            total.requests.to_string(),
            sessions,
            width,
        ))
        .child(headline(
            "Cache read",
            cache_share(total).map_or_else(|| "—".to_owned(), |share| format!("{share}%")),
            format!("{} of input", tokens(total.cache_read)),
            width,
        ))
}

/// The bar of `day`, `width` logical pixels across: a part for each of `series` and one for the rest, the first at the bottom, as tall as their share of `most`; resting on it says the day and what it came to.
fn day_bar(day: &UsageDay, series: &[String], measure: Measure, most: u64, width: f32) -> Node {
    let amounts = stack(day, series, measure);
    let whole: u64 = amounts.iter().sum();
    let parts = heights(&amounts, most, CHART_HEIGHT);
    node()
        .styles([
            flex(),
            flex_col(),
            justify_end(),
            flex_none(),
            w_px(width),
            h_px(CHART_HEIGHT),
        ])
        .children(
            parts
                .into_iter()
                .enumerate()
                .rev()
                .filter(|(_, tall)| *tall > 0.0)
                .map(|(place, tall)| {
                    node().styles([
                        flex_none(),
                        w_px(width),
                        h_px(tall.max(1.0)),
                        bg(series_tone(place)),
                    ])
                }),
        )
        .tooltip(format!(
            "{}: {}",
            day_label(&day.day),
            figure(whole, measure)
        ))
}

/// One entry of the chart's legend: a swatch of `tone` and the name it stands for.
fn legend_entry(name: &str, tone: Color) -> Node {
    node()
        .styles([flex(), items_center(), flex_none(), gap(1.5)])
        .child(node().styles([flex_none(), w_px(8), h_px(8), rounded(Radius::Sm), bg(tone)]))
        .child(line(name, 12, 1.4, Role::Muted))
}

/// The chart of `report` in `room` logical pixels: what it measures and the most a day came to, a bar for each day stacked by model, the first and last day under them, and which colour is which model.
fn chart(report: &UsageReport, room: f32) -> Node {
    let measure = measure(report);
    let series = series(report);
    let most = report
        .days
        .iter()
        .map(|day| stack(day, &series, measure).iter().sum::<u64>())
        .max()
        .unwrap_or(0);
    let width = bar_width(room, report.days.len());
    let title = match measure {
        Measure::Cost => "Spend per day",
        Measure::Tokens => "Tokens per day",
    };
    let others = report.models.len() > series.len();
    let ends = |day: Option<&UsageDay>| {
        line(
            day.map(|day| day_label(&day.day)).unwrap_or_default(),
            12,
            1.4,
            Role::Muted,
        )
        .styles([flex_none()])
    };
    node()
        .styles([flex(), flex_col(), flex_none(), gap(2), w_px(room)])
        .child(
            node()
                .styles([flex(), items_center(), justify_between(), gap(3)])
                .child(line(title, 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(
                    line(
                        format!("most {}", figure(most, measure)),
                        12,
                        1.4,
                        Role::Muted,
                    )
                    .styles([flex_none()]),
                ),
        )
        .child(
            node()
                .styles([
                    flex(),
                    items_end(),
                    justify_between(),
                    flex_none(),
                    w_px(room),
                    h_px(CHART_HEIGHT + 1.0),
                    border_b(),
                    border_color(Role::Border),
                ])
                .children(
                    report
                        .days
                        .iter()
                        .map(|day| day_bar(day, &series, measure, most, width)),
                ),
        )
        .child(
            node()
                .styles([flex(), items_center(), justify_between()])
                .child(ends(report.days.first()))
                .child(ends(report.days.last())),
        )
        .child(
            node()
                .styles([flex(), flex_wrap(), items_center(), gap(3)])
                .children(
                    series
                        .iter()
                        .enumerate()
                        .map(|(place, name)| legend_entry(name, series_tone(place))),
                )
                .children(others.then(|| legend_entry(OTHER, series_tone(MOST_SERIES)))),
        )
}

/// A row of a boxed list holding `row`, with room around it.
fn padded(row: Node) -> Node {
    node()
        .styles([flex(), flex_none(), px(3), pm_style::py(3)])
        .child(row)
}

/// How wide a row inside a boxed list is when the page is `room` logical pixels across.
fn row_width(room: f32) -> f32 {
    (room - 28.0).max(40.0)
}

/// What `totals` holds beside its headline figure, on one line: its tokens and its requests.
fn counts(totals: &UsageTotals) -> String {
    format!(
        "{} tokens · {}",
        tokens(totals.tokens()),
        requests(totals.requests)
    )
}

/// The models of `report`, one to a row in `room` logical pixels, the costliest first, each with a bar as long as its share of the costliest and in its colour of the chart.
fn model_rows(report: &UsageReport, room: f32) -> Vec<Node> {
    let measure = measure(report);
    let most = report
        .models
        .iter()
        .map(|model| amount(&model.totals, measure))
        .max()
        .unwrap_or(0);
    report
        .models
        .iter()
        .enumerate()
        .map(|(place, model)| {
            let unpriced = measure == Measure::Cost && model.totals.cost.is_none();
            let detail = if unpriced {
                format!("{} · no known prices", counts(&model.totals))
            } else {
                counts(&model.totals)
            };
            padded(
                BarRow::new(&model.model, spend_label(&model.totals), row_width(room))
                    .detail(detail)
                    .share(share_of(amount(&model.totals, measure), most))
                    .tone(series_tone(place))
                    .build(),
            )
        })
        .collect()
}

/// How much of `most` `amount` is, from nought to one; nothing when `most` is.
pub(crate) fn share_of(amount: u64, most: u64) -> f32 {
    if most == 0 {
        0.0
    } else {
        amount as f32 / most as f32
    }
}

/// The sessions of `report` that used most, one to a row in `room` logical pixels, each with its title, its project, what it used and a bar as long as its share of the first.
fn session_rows(report: &UsageReport, room: f32) -> Vec<Node> {
    let measure = measure(report);
    let most = report
        .sessions
        .iter()
        .map(|session| amount(&session.totals, measure))
        .max()
        .unwrap_or(0);
    report
        .sessions
        .iter()
        .take(TOP_SESSIONS)
        .map(|session| {
            padded(
                BarRow::new(
                    session_name(session),
                    spend_label(&session.totals),
                    row_width(room),
                )
                .detail(format!(
                    "{} · {}",
                    project_name(&session.project),
                    counts(&session.totals)
                ))
                .share(share_of(amount(&session.totals, measure), most))
                .build(),
            )
        })
        .collect()
}

/// What the machine `host` is called in a note: its name, or this machine.
fn host_name(host: Option<&str>) -> &str {
    host.unwrap_or("This machine")
}

/// The body of the usage page, `room` logical pixels across: what it is for, the choice of days, then the headline figures, the chart, the models and the top sessions; or that the servers are being asked, that nothing was used, or why a server could not say.
pub(crate) fn body(state: &WorkspaceState, page: Node, room: f32) -> Node {
    let usage = &state.settings_page.usage;
    let picked = RANGES
        .iter()
        .position(|days| *days == usage.days)
        .unwrap_or(RANGES.len());
    let status = if usage.loading {
        Some("Asking…".to_owned())
    } else if usage.reports.len() > 1 {
        Some(format!("Added up over {} servers", usage.reports.len()))
    } else {
        None
    };
    let page = page
        .child(paragraph(SUMMARY, room))
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_between(),
                    flex_wrap(),
                    flex_none(),
                    gap(2),
                ])
                .child(choices(&RANGE_NAMES, picked, |option| {
                    USAGE_RANGES.id(option)
                }))
                .children(status.map(|said| note(&said))),
        )
        .children(usage.errors.iter().map(|(host, reason)| {
            wrapped_note(&format!("{}: {reason}", host_name(host.as_deref())), room)
        }));
    if usage.reports.is_empty() {
        return if usage.loading {
            page.child(note("Reading usage…"))
        } else if usage.errors.is_empty() {
            page.child(note("No usage recorded yet"))
        } else {
            page
        };
    }
    let report = merged(&usage.reports);
    if report.total.is_empty() {
        return page.child(note("No usage recorded yet"));
    }
    let models = model_rows(&report, room);
    let sessions = session_rows(&report, room);
    page.child(headlines(&report, room))
        .child(chart(&report, room))
        .child(heading_only("Models", models.len()))
        .child(boxed(models))
        .child(heading_only("Top sessions", sessions.len()))
        .child(boxed(sessions))
}

/// What was spent over the days the page covers, in words, for the page's card on the Agents overview; nothing until a server reported.
pub(crate) fn count(state: &WorkspaceState) -> String {
    let usage = &state.settings_page.usage;
    if usage.reports.is_empty() {
        return String::new();
    }
    let report = merged(&usage.reports);
    if report.total.is_empty() {
        return String::new();
    }
    format!("{} in {} days", spend_label(&report.total), usage.days)
}

/// Makes the report the snapshots and the tests show: `days` days ending on the 17th of November 2026, three models of which the last has no known prices, and five sessions in two projects and a chat.
pub fn sample_report(days: u32) -> UsageReport {
    let models = ["claude-opus-4-5", "claude-haiku-4-5", "local/qwen3-coder"];
    let totals = |requests: u64, micros: Option<u64>| UsageTotals {
        requests,
        input: requests * 2_400,
        output: requests * 1_900,
        cache_read: requests * 61_000,
        cache_write: requests * 5_200,
        cost: micros,
        missed: micros.unwrap_or(0) / 14,
    };
    let mut report = UsageReport::default();
    for back in (0..days).rev() {
        let date = 17_i64 - i64::from(back);
        let (month, date) = match date {
            1.. => (11, date),
            -30..=0 => (10, date + 31),
            -60..=-31 => (9, date + 61),
            _ => (8, date + 92),
        };
        let beat = u64::from(back % 7);
        let rest = back % 7 == 5 || back % 11 == 6;
        let mut day = UsageDay {
            day: format!("2026-{month:02}-{date:02}"),
            models: Vec::new(),
        };
        if !rest {
            let asked = [
                (14 + beat * 9 + u64::from(back % 3) * 6, Some(180_000_u64)),
                (22 + beat * 5, Some(21_000)),
                (u64::from(back % 4) * 8, None),
            ];
            for (name, (requests, price)) in models.iter().zip(asked) {
                if requests == 0 {
                    continue;
                }
                let part = totals(requests, price.map(|price| price * requests));
                add_model(&mut day.models, name, &part);
                add_model(&mut report.models, name, &part);
                report.total.add(&part);
            }
        }
        report.days.push(day);
    }
    let named = [
        (
            41,
            "Rework the diff renderer for long files",
            "/home/ada/code/pandemonium",
            38,
        ),
        (
            44,
            "Usage dashboard in settings",
            "/home/ada/code/pandemonium",
            24,
        ),
        (
            39,
            "Port the wire format to streaming",
            "/home/ada/code/oh-my-crab",
            17,
        ),
        (46, "Why does the cache miss after a rewind?", "", 12),
        (
            37,
            "Flaky terminal resize test",
            "/home/ada/code/pandemonium",
            9,
        ),
    ];
    let priced = report.total.cost.unwrap_or(0);
    report.sessions = named
        .into_iter()
        .map(|(session, title, project, percent)| UsageSession {
            session,
            title: title.to_owned(),
            project: project.to_owned(),
            updated: 1_794_900_000_000 - session * 3_600_000,
            totals: totals(
                report.total.requests * percent / 100,
                Some(priced * percent / 100),
            ),
        })
        .collect();
    report
}

#[cfg(test)]
/// Tests for the usage page: asking the servers, taking their answers, adding them up, and the figures and bars the page draws.
mod tests {
    use super::*;
    use crate::settings::Section;

    /// Totals of `requests` requests costing `cost`, with a hundred tokens of each kind to a request.
    fn totals(requests: u64, cost: Option<u64>) -> UsageTotals {
        UsageTotals {
            requests,
            input: requests * 100,
            output: requests * 100,
            cache_read: requests * 100,
            cache_write: requests * 100,
            cost,
            missed: 0,
        }
    }

    /// What the model `name` was asked.
    fn model(name: &str, requests: u64, cost: Option<u64>) -> UsageModel {
        UsageModel {
            model: name.to_owned(),
            totals: totals(requests, cost),
        }
    }

    /// A report of one day holding `models`, with them as its models and their sum as its total, and one session.
    fn report(day: &str, models: Vec<UsageModel>, session: u64) -> UsageReport {
        let mut total = UsageTotals::default();
        for model in &models {
            total.add(&model.totals);
        }
        UsageReport {
            days: vec![UsageDay {
                day: day.to_owned(),
                models: models.clone(),
            }],
            models,
            sessions: vec![UsageSession {
                session,
                totals: total.clone(),
                ..UsageSession::default()
            }],
            total,
        }
    }

    /// The state with the settings window open on the usage page and nothing waiting to be sent.
    fn open() -> WorkspaceState {
        let mut state = WorkspaceState::new(&[])
            .with_settings()
            .settings_pressed(Section::Usage.id());
        state.take_outbox();
        state
    }

    /// Opening the page asks the servers for thirty days and says it is waiting.
    #[test]
    fn opening_the_page_asks_for_the_report() {
        let mut state = WorkspaceState::new(&[]).with_settings();
        state.take_outbox();
        let mut state = state.settings_pressed(Section::Usage.id());
        assert_eq!(state.settings, Some(Section::Usage));
        assert!(state.settings_page.usage.loading);
        assert!(
            state
                .take_outbox()
                .contains(&Outgoing::UsageReport { days: 30 })
        );
    }

    /// Picking another run of days drops the reports held and asks for that run; an answer about the old run is dropped, and one about the new run is kept.
    #[test]
    fn picking_a_range_asks_again() {
        let state = open().with_usage_report(
            None,
            30,
            Ok(report("2026-11-17", vec![model("a", 1, Some(5))], 1)),
        );
        assert_eq!(state.settings_page.usage.reports.len(), 1);
        assert!(!state.settings_page.usage.loading);
        let mut state = state.settings_pressed(USAGE_RANGES.id(0).unwrap());
        assert_eq!(state.settings_page.usage.days, 7);
        assert!(state.settings_page.usage.reports.is_empty());
        assert!(
            state
                .take_outbox()
                .contains(&Outgoing::UsageReport { days: 7 })
        );
        let state = state.with_usage_report(None, 30, Ok(UsageReport::default()));
        assert!(state.settings_page.usage.reports.is_empty());
        assert!(state.settings_page.usage.loading);
        let state = state.with_usage_report(None, 7, Ok(UsageReport::default()));
        assert_eq!(state.settings_page.usage.reports.len(), 1);
        assert!(!state.settings_page.usage.loading);
    }

    /// A server's newer answer takes the place of its older one, and a server that could not answer is named with why beside the ones that could.
    #[test]
    fn each_server_has_one_answer() {
        let lab = Some("lab".to_owned());
        let state = open()
            .with_usage_report(None, 30, Ok(UsageReport::default()))
            .with_usage_report(lab.clone(), 30, Err("too old".to_owned()))
            .with_usage_report(
                None,
                30,
                Ok(report("2026-11-17", vec![model("a", 2, None)], 1)),
            );
        let usage = &state.settings_page.usage;
        assert_eq!(usage.reports.len(), 1);
        assert_eq!(usage.reports[0].report.total.requests, 2);
        assert_eq!(usage.errors, [(lab.clone(), "too old".to_owned())]);
        let state = state.with_usage_report(lab, 30, Ok(UsageReport::default()));
        assert!(state.settings_page.usage.errors.is_empty());
        assert_eq!(state.settings_page.usage.reports.len(), 2);
    }

    /// The reports of two servers add up by day and by model, keep the days in order and every session, the costliest first.
    #[test]
    fn reports_of_several_servers_add_up() {
        let here = HostReport {
            host: None,
            report: report(
                "2026-11-17",
                vec![model("a", 1, Some(100)), model("b", 1, Some(40))],
                1,
            ),
        };
        let mut there = report("2026-11-17", vec![model("b", 2, Some(300))], 1);
        there.days.insert(
            0,
            UsageDay {
                day: "2026-11-16".to_owned(),
                models: Vec::new(),
            },
        );
        let whole = merged(&[
            here,
            HostReport {
                host: Some("lab".to_owned()),
                report: there,
            },
        ]);
        let days: Vec<&str> = whole.days.iter().map(|day| day.day.as_str()).collect();
        assert_eq!(days, ["2026-11-16", "2026-11-17"]);
        assert_eq!(whole.days[1].models[0].model, "b");
        assert_eq!(whole.days[1].models[0].totals.cost, Some(340));
        assert_eq!(whole.models[0].model, "b");
        assert_eq!(whole.models[1].totals.cost, Some(100));
        assert_eq!(whole.total.cost, Some(440));
        assert_eq!(whole.total.requests, 4);
        assert_eq!(whole.sessions.len(), 2);
        assert_eq!(whole.sessions[0].totals.cost, Some(300));
        assert!(merged(&[]).total.is_empty());
    }

    /// The bars measure spend when anything has a cost and tokens when nothing has, and the figures are written to match.
    #[test]
    fn the_measure_falls_back_to_tokens() {
        let priced = report("2026-11-17", vec![model("a", 1, Some(2_500_000))], 1);
        let free = report("2026-11-17", vec![model("a", 5, None)], 1);
        assert_eq!(measure(&priced), Measure::Cost);
        assert_eq!(measure(&free), Measure::Tokens);
        assert_eq!(amount(&priced.total, Measure::Cost), 2_500_000);
        assert_eq!(amount(&free.total, Measure::Tokens), 2_000);
        assert_eq!(figure(2_500_000, Measure::Cost), "$2.50");
        assert_eq!(figure(2_000, Measure::Tokens), "2k tokens");
        assert_eq!(spend_label(&priced.total), "$2.50");
        assert_eq!(spend_label(&free.total), "2k tokens");
    }

    /// The share read from the cache is of the input sent, and there is none without input.
    #[test]
    fn the_cache_share_is_of_the_input() {
        let read = UsageTotals {
            input: 10,
            cache_read: 80,
            cache_write: 10,
            output: 500,
            ..UsageTotals::default()
        };
        assert_eq!(cache_share(&read), Some(80));
        assert_eq!(cache_share(&UsageTotals::default()), None);
    }

    /// Days, projects and sessions are named in short.
    #[test]
    fn names_are_short() {
        assert_eq!(day_label("2026-11-17"), "Nov 17");
        assert_eq!(day_label("2026-01-05"), "Jan 5");
        assert_eq!(day_label("2026-13-05"), "2026-13-05");
        assert_eq!(day_label("soon"), "soon");
        assert_eq!(project_name("/home/ada/code/app/"), "app");
        assert_eq!(project_name("C:\\code\\app"), "app");
        assert_eq!(project_name(""), "Chat");
        let mut session = UsageSession {
            session: 7,
            ..UsageSession::default()
        };
        assert_eq!(session_name(&session), "Session 7");
        session.title = "Fix it".to_owned();
        assert_eq!(session_name(&session), "Fix it");
    }

    /// A day's bar has a part for each model with a colour of its own and one for the rest, each as tall as its share of the tallest day.
    #[test]
    fn a_bar_stacks_its_models() {
        let day = UsageDay {
            day: "2026-11-17".to_owned(),
            models: vec![
                model("a", 1, Some(50)),
                model("b", 1, Some(30)),
                model("c", 1, Some(15)),
                model("d", 1, Some(5)),
            ],
        };
        let series = vec!["a".to_owned(), "b".to_owned()];
        let amounts = stack(&day, &series, Measure::Cost);
        assert_eq!(amounts, [50, 30, 20]);
        assert_eq!(heights(&amounts, 200, 100.0), [25.0, 15.0, 10.0]);
        assert_eq!(heights(&amounts, 0, 100.0), [0.0, 0.0, 0.0]);
        assert_eq!(stack(&day, &series, Measure::Tokens), [400, 400, 800]);
        assert_eq!(share_of(1, 4), 0.25);
        assert_eq!(share_of(1, 0), 0.0);
    }

    /// The bars fit the room side by side, never thinner than a pixel nor wider than the widest.
    #[test]
    fn bars_fit_the_room() {
        assert_eq!(bar_width(366.0, 90), 3.0);
        assert_eq!(bar_width(366.0, 30), 10.0);
        assert_eq!(bar_width(366.0, 7), WIDEST_BAR);
        assert_eq!(bar_width(40.0, 90), 1.0);
        assert_eq!(bar_width(366.0, 0), 0.0);
    }

    /// The sample the snapshots show has every day of its run, three models of which one has no prices, five sessions, and a total that is the sum of its models.
    #[test]
    fn the_sample_report_is_whole() {
        for days in RANGES {
            let sample = sample_report(days);
            assert_eq!(sample.days.len(), days as usize);
            assert_eq!(sample.days.last().unwrap().day, "2026-11-17");
            assert!(sample.days.windows(2).all(|pair| pair[0].day < pair[1].day));
            assert_eq!(sample.models.len(), 3);
            assert_eq!(sample.sessions.len(), 5);
            let mut sum = UsageTotals::default();
            for model in &sample.models {
                sum.add(&model.totals);
            }
            assert_eq!(sum, sample.total);
            assert_eq!(measure(&sample), Measure::Cost);
        }
        assert_eq!(sample_report(30).days[0].day, "2026-10-19");
        assert_eq!(sample_report(90).days[0].day, "2026-08-20");
    }

    /// The overview card says what was spent once a server reported, and nothing before.
    #[test]
    fn the_card_says_what_was_spent() {
        let state = open();
        assert_eq!(count(&state), "");
        let state = state.with_usage_report(
            None,
            30,
            Ok(report(
                "2026-11-17",
                vec![model("a", 1, Some(2_500_000))],
                1,
            )),
        );
        assert_eq!(count(&state), "$2.50 in 30 days");
    }
}

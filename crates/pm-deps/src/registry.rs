//! Asking registries for the latest version of a dependency, kept for a while, spaced out and never fatal.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::dependency::Outdated;
use crate::ecosystem::{Ecosystem, ecosystem_for};
use crate::version::{Version, rewritten};

/// How long what a registry said is kept before it is asked again.
const FRESH: Duration = Duration::from_secs(60 * 60);

/// How long the failure of a lookup is kept, so that a registry that is down is not asked again for every edit.
const FAILED: Duration = Duration::from_secs(5 * 60);

/// The least time between two requests to the same registry.
const SPACING: Duration = Duration::from_millis(100);

/// How long one call to [`Registry::outdated`] spends asking.
const BUDGET: Duration = Duration::from_secs(8);

/// How long one request waits for an answer.
const PATIENCE: Duration = Duration::from_secs(5);

/// How many dependencies of one manifest are looked up.
const MOST: usize = 80;

/// How many requests run at once.
const WORKERS: usize = 4;

/// What was learnt of one dependency, and when.
struct Learnt {
    /// When it was learnt.
    at: Instant,
    /// The latest version, or nothing when the lookup failed.
    latest: Option<String>,
}

/// The registries' answers, kept, and the pace at which they are asked.
#[derive(Default)]
pub struct Registry {
    /// What was learnt, by ecosystem and dependency.
    learnt: Mutex<HashMap<(&'static str, String), Learnt>>,
    /// When each ecosystem's registry was asked last.
    asked: Mutex<HashMap<&'static str, Instant>>,
}

impl Registry {
    /// The latest version of `name` in the registry of `ecosystem`, from what was kept when that is recent, or nothing when the registry does not say.
    fn latest(&self, ecosystem: &dyn Ecosystem, name: &str) -> Option<String> {
        let key = (ecosystem.name(), name.to_owned());
        if let Some(held) = self.learnt.lock().ok().and_then(|learnt| {
            learnt.get(&key).and_then(|held| {
                let keeps = if held.latest.is_some() { FRESH } else { FAILED };
                (held.at.elapsed() < keeps).then(|| held.latest.clone())
            })
        }) {
            return held;
        }
        let latest = self.fetch(ecosystem, name);
        if let Ok(mut learnt) = self.learnt.lock() {
            learnt.insert(
                key,
                Learnt {
                    at: Instant::now(),
                    latest: latest.clone(),
                },
            );
        }
        latest
    }

    /// Asks the registry of `ecosystem` about `name`, after the pause that spaces its requests out.
    fn fetch(&self, ecosystem: &dyn Ecosystem, name: &str) -> Option<String> {
        let wait = {
            let mut asked = self.asked.lock().ok()?;
            let now = Instant::now();
            let turn = asked
                .get(ecosystem.name())
                .map_or(now, |last| (*last + SPACING).max(now));
            asked.insert(ecosystem.name(), turn);
            turn - now
        };
        std::thread::sleep(wait);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(PATIENCE))
            .user_agent("pandemonium (https://github.com/JoakimCarlsson/pandemonium)")
            .build()
            .into();
        let answer = agent
            .get(ecosystem.address(name))
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()?;
        ecosystem.latest_in(&answer)
    }

    /// The dependencies of `text`, the manifest at `path`, whose version is behind the latest their registry has, each with the requirement that asks for the latest; none when `path` is not a manifest, and a dependency whose lookup fails or takes too long is left out.
    pub fn outdated(&self, path: &str, text: &str) -> Vec<Outdated> {
        let Some(ecosystem) = ecosystem_for(path) else {
            return Vec::new();
        };
        let dependencies: Vec<_> = ecosystem
            .dependencies(text)
            .into_iter()
            .filter(|dependency| Version::parse(&dependency.requirement).is_some())
            .take(MOST)
            .collect();
        let deadline = Instant::now() + BUDGET;
        let next = AtomicUsize::new(0);
        let answers: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..WORKERS.min(dependencies.len()) {
                scope.spawn(|| {
                    loop {
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some(dependency) = dependencies.get(at) else {
                            return;
                        };
                        if Instant::now() >= deadline {
                            return;
                        }
                        if let Some(latest) = self.latest(ecosystem, &dependency.name)
                            && let Ok(mut answers) = answers.lock()
                        {
                            answers.push((at, latest));
                        }
                    }
                });
            }
        });
        let mut answers = answers.into_inner().unwrap_or_default();
        answers.sort();
        answers
            .into_iter()
            .filter_map(|(at, latest)| {
                let dependency = dependencies[at].clone();
                let held = Version::parse(&dependency.requirement)?;
                held.is_behind(&Version::parse(&latest)?).then(|| Outdated {
                    replacement: rewritten(&dependency.requirement, &latest),
                    dependency,
                    latest,
                })
            })
            .collect()
    }
}

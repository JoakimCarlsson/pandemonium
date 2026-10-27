//! Asking registries for the latest version of a dependency, kept for a while, spaced out and never fatal.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::dependency::Dependency;
use crate::dependency::Outdated;
use crate::provider::Provider;
use crate::version::Version;

/// How long what a registry said is kept before it is asked again, unless the language says another.
const FRESH: Duration = Duration::from_secs(60 * 60);

/// How long the failure of a lookup is kept, so that a registry that is down is not asked again for every edit.
const FAILED: Duration = Duration::from_secs(5 * 60);

/// The least time between two requests to the same registry, unless the language says another.
const SPACING: Duration = Duration::from_millis(100);

/// How long one call to [`Registry::outdated`] spends asking, unless the language says another.
const BUDGET: Duration = Duration::from_secs(8);

/// How long one request waits for an answer.
const PATIENCE: Duration = Duration::from_secs(5);

/// How many dependencies of one manifest are looked up, unless the language says another.
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
    /// What was learnt, by registry and dependency.
    learnt: Mutex<HashMap<(String, String), Learnt>>,
    /// When each registry was asked last.
    asked: Mutex<HashMap<String, Instant>>,
}

impl Registry {
    /// The latest version of `name` in the registry of `provider`, from what was kept when that is recent, or nothing when the registry does not say.
    fn latest(&self, provider: &Provider, name: &str) -> Option<String> {
        let fresh = provider
            .fresh_minutes
            .map_or(FRESH, |minutes| Duration::from_secs(minutes * 60));
        self.lookup_for(&provider.url, name, fresh, || self.fetch(provider, name))
    }

    /// The latest version of `name` in the registry called `registry`, from what was kept when that is recent, and otherwise what `fetch` finds, which is kept; for a registry that something other than a [`Provider`] answers for.
    pub fn lookup(
        &self,
        registry: &str,
        name: &str,
        fetch: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        self.lookup_for(registry, name, FRESH, fetch)
    }

    /// As [`Registry::lookup`], keeping what was found for `fresh` before asking again.
    fn lookup_for(
        &self,
        registry: &str,
        name: &str,
        fresh: Duration,
        fetch: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let key = (registry.to_owned(), name.to_owned());
        if let Some(held) = self.learnt.lock().ok().and_then(|learnt| {
            learnt.get(&key).and_then(|held| {
                let keeps = if held.latest.is_some() { fresh } else { FAILED };
                (held.at.elapsed() < keeps).then(|| held.latest.clone())
            })
        }) {
            return held;
        }
        let latest = fetch();
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

    /// Asks the registry of `provider` about `name`, after the pause that spaces its requests out.
    fn fetch(&self, provider: &Provider, name: &str) -> Option<String> {
        let spacing = provider.spacing_ms.map_or(SPACING, Duration::from_millis);
        let wait = {
            let mut asked = self.asked.lock().ok()?;
            let now = Instant::now();
            let turn = asked
                .get(&provider.url)
                .map_or(now, |last| (*last + spacing).max(now));
            asked.insert(provider.url.clone(), turn);
            turn - now
        };
        std::thread::sleep(wait);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(PATIENCE))
            .user_agent("pandemonium (https://github.com/JoakimCarlsson/pandemonium)")
            .build()
            .into();
        let answer = agent
            .get(provider.address(name))
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()?;
        provider.latest_in(&answer)
    }

    /// The dependencies among `dependencies`, those the manifest in `folder` names, whose version is behind the latest the registry of `provider` has, each with the requirement that asks for the latest; a dependency that is private, whose lookup fails or takes too long is left out.
    pub fn outdated(
        &self,
        provider: &Provider,
        folder: &std::path::Path,
        dependencies: Vec<Dependency>,
    ) -> Vec<Outdated> {
        let Ok(pattern) = regex::Regex::new(&provider.requirement) else {
            return Vec::new();
        };
        let settings = provider.settings(folder);
        let dependencies: Vec<Dependency> = dependencies
            .into_iter()
            .filter(|dependency| Version::parse(&pattern, &dependency.requirement).is_some())
            .filter(|dependency| !provider.skips(&dependency.name, &settings))
            .take(provider.most.unwrap_or(MOST))
            .collect();
        let deadline = Instant::now() + provider.budget_seconds.map_or(BUDGET, Duration::from_secs);
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
                        if let Some(latest) = self.latest(provider, &dependency.name)
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
                let held = Version::parse(&pattern, &dependency.requirement)?;
                let newest = Version::parse(&pattern, &latest)?;
                held.is_behind(&newest).then(|| Outdated {
                    replacement: held.rewritten(&newest),
                    dependency,
                    latest,
                })
            })
            .collect()
    }
}

//! Stopping, removing and describing the environments this editor owns.

use super::Shared;
use super::journal::Journal;
use crate::engine;
use crate::event::{Phase, Sink, State};
use crate::failure::{Failure, Stage};
use crate::identity::Identity;
use crate::plan::{ENVIRONMENT, OWNER};
use std::sync::Arc;

/// Releases the forwards and the machine connection of `id`, keeping its container.
pub(super) fn release(shared: &Shared, id: &str) {
    let (forwarder, host) = {
        let mut runtimes = shared.runtimes.lock().unwrap();
        match runtimes.get_mut(id) {
            Some(runtime) => {
                runtime.forwards.lock().unwrap().clear();
                (runtime.forwarder.take(), Some(runtime.host.clone()))
            }
            None => (None, None),
        }
    };
    drop(forwarder);
    if let Some(host) = host {
        host.disconnect();
    }
}

/// Stops the container of `identity`, after releasing everything that reaches into it.
pub(super) fn stop(shared: &Arc<Shared>, identity: &Identity, sink: &Sink) -> Result<(), Failure> {
    let id = identity.id();
    let _claim = shared.claim(&id)?;
    let state = shared
        .runtimes
        .lock()
        .unwrap()
        .get(&id)
        .map(|runtime| runtime.state.clone())
        .unwrap_or_default();
    let journal = Journal::start(shared.records.log(&id), false, sink.clone(), state);
    journal.phase(Phase::Stopping);
    release(shared, &id);
    let result: Result<(), Failure> = (|| {
        let names = shared
            .engine
            .containers_with(&format!("{ENVIRONMENT}={id}"))
            .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
        for name in owned(shared, names) {
            let mut command = shared.engine.command();
            command.args(["stop", &name]);
            engine::stream(command, "stop", &mut |source, line| {
                journal.log(source, line)
            })
            .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
        }
        Ok(())
    })();
    match &result {
        Ok(()) => journal.state(State::Stopped),
        Err(failure) => journal.state(State::Failed(failure.message.clone())),
    }
    result.map_err(|failure| failure.with_log(journal.path().to_path_buf()))
}

/// Takes the environment of `identity` away for good: its container, volumes, built image and record.
pub(super) fn retire(
    shared: &Arc<Shared>,
    identity: &Identity,
    sink: &Sink,
) -> Result<(), Failure> {
    let id = identity.id();
    let _claim = shared.claim(&id)?;
    let state = shared
        .runtimes
        .lock()
        .unwrap()
        .get(&id)
        .map(|runtime| runtime.state.clone())
        .unwrap_or_default();
    let journal = Journal::start(shared.records.log(&id), false, sink.clone(), state);
    journal.phase(Phase::Removing);
    release(shared, &id);
    remove(shared, &journal, &id)?;
    let image = format!(
        "pandemonium-devcontainer/{}:latest",
        identity.container_name()
    );
    if shared
        .engine
        .image_label(&image, OWNER)
        .is_some_and(|owner| owner == "pandemonium")
    {
        let mut command = shared.engine.command();
        command.args(["image", "rm", &image]);
        engine::stream(command, "remove", &mut |source, line| {
            journal.log(source, line)
        })
        .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
    }
    shared.records.forget(&id);
    shared.runtimes.lock().unwrap().remove(&id);
    journal.state(State::Absent);
    Ok(())
}

/// Removes the container and volumes owned by `id`, and nothing else.
pub(super) fn remove(shared: &Shared, journal: &Journal, id: &str) -> Result<(), Failure> {
    let label = format!("{ENVIRONMENT}={id}");
    let names = shared
        .engine
        .containers_with(&label)
        .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
    for name in owned(shared, names) {
        let mut command = shared.engine.command();
        command.args(["rm", "-f", "-v", &name]);
        engine::stream(command, "remove", &mut |source, line| {
            journal.log(source, line)
        })
        .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
    }
    let volumes = shared
        .engine
        .volumes_with(&label)
        .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
    for volume in volumes {
        let mut command = shared.engine.command();
        command.args(["volume", "rm", &volume]);
        engine::stream(command, "remove", &mut |source, line| {
            journal.log(source, line)
        })
        .map_err(|error| Failure::new(Stage::Stop, error.to_string()))?;
    }
    Ok(())
}

/// The containers among `names` that carry this editor's owner label.
fn owned(shared: &Shared, names: Vec<String>) -> Vec<String> {
    names
        .into_iter()
        .filter(|name| {
            shared
                .engine
                .inspect(name)
                .ok()
                .flatten()
                .is_some_and(|found| {
                    found
                        .labels
                        .get(OWNER)
                        .is_some_and(|owner| owner == "pandemonium")
                })
        })
        .collect()
}

/// Where the environment of `identity` is now.
pub(super) fn state(shared: &Arc<Shared>, identity: &Identity) -> State {
    let id = identity.id();
    if shared.busy.lock().unwrap().contains(&id) {
        return shared
            .runtimes
            .lock()
            .unwrap()
            .get(&id)
            .map_or(State::Absent, |runtime| {
                runtime.state.lock().unwrap().clone()
            });
    }
    if let Some(runtime) = shared.runtimes.lock().unwrap().get(&id) {
        return runtime.state.lock().unwrap().clone();
    }
    match shared
        .engine
        .containers_with(&format!("{ENVIRONMENT}={id}"))
    {
        Ok(names) if !names.is_empty() => State::Stopped,
        _ => State::Absent,
    }
}

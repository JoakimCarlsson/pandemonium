//! One shared value and the observers that are told when it changes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Identifies one observer, so it can be removed again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Subscription(u64);

/// What is told about a change: the value as it now is.
type Observer<T> = Rc<dyn Fn(&T)>;

/// The part of a state every handle shares.
struct Shared<T> {
    /// The value.
    value: RefCell<T>,
    /// How many times the value has changed.
    version: Cell<u64>,
    /// The next observer identity to hand out.
    next: Cell<u64>,
    /// The observers, in the order they subscribed.
    observers: RefCell<Vec<(Subscription, Observer<T>)>>,
}

/// A value shared by every clone of its handle, which reports changes to its
/// observers.
pub struct State<T> {
    /// What the handles share.
    shared: Rc<Shared<T>>,
}

impl<T> Clone for State<T> {
    /// Another handle to the same value.
    fn clone(&self) -> Self {
        Self {
            shared: Rc::clone(&self.shared),
        }
    }
}

impl<T> State<T> {
    /// A state holding `value`, at version zero.
    pub fn new(value: T) -> Self {
        Self {
            shared: Rc::new(Shared {
                value: RefCell::new(value),
                version: Cell::new(0),
                next: Cell::new(0),
                observers: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Reads the value through `read` and returns what it returns.
    pub fn read<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.shared.value.borrow())
    }

    /// Replaces the value and tells the observers.
    pub fn set(&self, value: T) {
        self.update(|current| *current = value);
    }

    /// Changes the value in place through `change`, then tells the observers.
    pub fn update(&self, change: impl FnOnce(&mut T)) {
        change(&mut self.shared.value.borrow_mut());
        self.shared.version.set(self.shared.version.get() + 1);
        let observers: Vec<Observer<T>> = self
            .shared
            .observers
            .borrow()
            .iter()
            .map(|(_, observer)| Rc::clone(observer))
            .collect();
        let value = self.shared.value.borrow();
        for observer in observers {
            observer(&value);
        }
    }

    /// How many times the value has changed since the state was made.
    pub fn version(&self) -> u64 {
        self.shared.version.get()
    }

    /// Calls `observer` with the value after every change, until the returned
    /// subscription is passed to [`State::unsubscribe`].
    pub fn subscribe(&self, observer: impl Fn(&T) + 'static) -> Subscription {
        let subscription = Subscription(self.shared.next.get());
        self.shared.next.set(subscription.0 + 1);
        self.shared
            .observers
            .borrow_mut()
            .push((subscription, Rc::new(observer)));
        subscription
    }

    /// Stops telling the observer behind `subscription` about changes.
    pub fn unsubscribe(&self, subscription: Subscription) {
        self.shared
            .observers
            .borrow_mut()
            .retain(|(held, _)| *held != subscription);
    }
}

impl<T: Clone> State<T> {
    /// A copy of the value.
    pub fn get(&self) -> T {
        self.read(T::clone)
    }
}

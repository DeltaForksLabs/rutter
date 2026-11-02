// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache-2.0.

use std::{fmt, sync::Arc};

/// A cloneable, owned change callback for a native controlled widget.
///
/// Captures must be owned (`'static`) and `Send + Sync`, but need not be `Copy`
/// or `Clone`. Cloning shares the callback; native widget dispatch invokes it on
/// the UI thread. Direct [`Self::call`] invocations do not enforce thread affinity.
/// The thread-safety bounds preserve `Send` and `Sync` for numeric configurations,
/// but captured trait objects do not promise unwind safety.
///
/// Sharing a callback does not refresh its capture or authorize its messages.
/// Capture immutable identity/generation snapshots and validate them again in
/// the application's update boundary. Rebuilding a view can install a new
/// callback, but retained clones and already-emitted messages keep their old
/// context. Avoid ownership cycles in captured application services.
pub struct ChangeCallback<T, Msg>(ChangeCallbackKind<T, Msg>);

enum ChangeCallbackKind<T, Msg> {
    Function(fn(T) -> Msg),
    Captured(Arc<dyn Fn(T) -> Msg + Send + Sync>),
}

impl<T, Msg> ChangeCallback<T, Msg> {
    /// Owns a callback, sharing its capture when this value is cloned.
    pub fn new(on_change: impl Fn(T) -> Msg + Send + Sync + 'static) -> Self {
        Self(ChangeCallbackKind::Captured(Arc::new(on_change)))
    }

    pub(crate) fn from_function(on_change: fn(T) -> Msg) -> Self {
        Self(ChangeCallbackKind::Function(on_change))
    }

    /// Produces a message using this callback's captured context.
    pub fn call(&self, value: T) -> Msg {
        match &self.0 {
            ChangeCallbackKind::Function(callback) => callback(value),
            ChangeCallbackKind::Captured(callback) => callback(value),
        }
    }
}

impl<T, Msg> Clone for ChangeCallback<T, Msg> {
    fn clone(&self) -> Self {
        match &self.0 {
            ChangeCallbackKind::Function(callback) => Self::from_function(*callback),
            ChangeCallbackKind::Captured(callback) => {
                Self(ChangeCallbackKind::Captured(Arc::clone(callback)))
            }
        }
    }
}

impl<T, Msg> fmt::Debug for ChangeCallback<T, Msg> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ChangeCallback(..)")
    }
}

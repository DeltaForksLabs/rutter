// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;

use super::Widget;

/// A non-zero, application-owned identity for one interactive virtual item.
///
/// ```rust
/// use rutter::VirtualItemKey;
///
/// assert_eq!(VirtualItemKey::new(42).unwrap().get(), 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualItemKey(NonZeroU64);

impl VirtualItemKey {
    /// Validates a key used to retain one virtual item's descendant state.
    ///
    /// ```rust
    /// use rutter::VirtualItemKey;
    ///
    /// assert!(VirtualItemKey::new(7).is_ok());
    /// ```
    pub fn new(value: u64) -> Result<Self, VirtualItemKeyError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(VirtualItemKeyError::Zero { value })
    }

    /// Returns the application-owned numeric key.
    ///
    /// ```rust
    /// use rutter::VirtualItemKey;
    ///
    /// assert_eq!(VirtualItemKey::new(9).unwrap().get(), 9);
    /// ```
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Explains why a virtual item key cannot be used for retained identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualItemKeyError {
    Zero { value: u64 },
}

impl fmt::Display for VirtualItemKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero { value } => write!(
                formatter,
                "invalid virtual item key {value}, expected a non-zero application-owned u64"
            ),
        }
    }
}

impl std::error::Error for VirtualItemKeyError {}

/// Explains why a keyed virtual collection cannot retain item identity safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyedVirtualItemsError {
    DuplicateKey {
        key: VirtualItemKey,
        first_index: usize,
        duplicate_index: usize,
    },
}

impl fmt::Display for KeyedVirtualItemsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey {
                key,
                first_index,
                duplicate_index,
            } => write!(
                formatter,
                "duplicate virtual item key {} at index {duplicate_index}, expected unique keys; first seen at index {first_index}",
                key.get()
            ),
        }
    }
}

impl std::error::Error for KeyedVirtualItemsError {}

/// Validated keys and a lazy widget builder for an interactive virtual collection.
///
/// The builder can be evaluated for rendering, hit testing, focus, and
/// accessibility. It must therefore return the same widget structure for a
/// given item until the next application view update.
///
/// ```rust
/// use rutter::{KeyedVirtualItems, VirtualItemKey, Widget};
/// use taffy::prelude::Style;
///
/// let keys = [VirtualItemKey::new(1).unwrap()];
/// let build = |_| Some(Widget::Button { text: "Open", on_press: (), style: Style::default(), color: None, variant: Default::default() });
/// let items = KeyedVirtualItems::try_new(&keys, &build).unwrap();
/// assert_eq!(items.len(), 1);
/// ```
#[derive(Clone, Copy)]
pub struct KeyedVirtualItems<'a, Msg> {
    keys: &'a [VirtualItemKey],
    build: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
}

impl<'a, Msg> KeyedVirtualItems<'a, Msg> {
    /// Validates unique item keys before enabling interactive virtualization.
    ///
    /// ```rust
    /// use rutter::{KeyedVirtualItems, VirtualItemKey, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let keys = [VirtualItemKey::new(1).unwrap()];
    /// let build = |_| Some(Widget::Button { text: "Run", on_press: (), style: Style::default(), color: None, variant: Default::default() });
    /// assert!(KeyedVirtualItems::try_new(&keys, &build).is_ok());
    /// ```
    pub fn try_new(
        keys: &'a [VirtualItemKey],
        build: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
    ) -> Result<Self, KeyedVirtualItemsError> {
        validate_keyed_virtual_items(keys)?;
        Ok(Self { keys, build })
    }

    /// Returns the number of keyed items without materializing child widgets.
    ///
    /// ```rust
    /// use rutter::{KeyedVirtualItems, VirtualItemKey, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let keys = [VirtualItemKey::new(1).unwrap()];
    /// let build = |_| Some(Widget::Button { text: "Run", on_press: (), style: Style::default(), color: None, variant: Default::default() });
    /// let items = KeyedVirtualItems::try_new(&keys, &build).unwrap();
    /// assert_eq!(items.len(), 1);
    /// ```
    pub const fn len(&self) -> usize {
        self.keys.len()
    }

    /// Reports whether the collection has no keyed items.
    ///
    /// ```rust
    /// use rutter::{KeyedVirtualItems, VirtualItemKey, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let keys: [VirtualItemKey; 0] = [];
    /// let build = |_| Some(Widget::Button { text: "Run", on_press: (), style: Style::default(), color: None, variant: Default::default() });
    /// assert!(KeyedVirtualItems::try_new(&keys, &build).unwrap().is_empty());
    /// ```
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub(crate) fn key_at(&self, index: usize) -> Option<VirtualItemKey> {
        self.keys.get(index).copied()
    }

    pub(crate) fn build_item(&self, index: usize) -> Option<Widget<'a, Msg>> {
        self.key_at(index)?;
        (self.build)(index)
    }
}

fn validate_keyed_virtual_items(keys: &[VirtualItemKey]) -> Result<(), KeyedVirtualItemsError> {
    let mut first_indices = HashMap::with_capacity(keys.len());
    for (index, key) in keys.iter().copied().enumerate() {
        if let Some(first_index) = first_indices.insert(key, index) {
            return Err(KeyedVirtualItemsError::DuplicateKey {
                key,
                first_index,
                duplicate_index: index,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::Style;

    #[test]
    fn zero_key_is_rejected_with_the_offending_value() {
        assert_eq!(
            VirtualItemKey::new(0),
            Err(VirtualItemKeyError::Zero { value: 0 })
        );
    }

    #[test]
    fn duplicate_key_reports_both_item_indices() {
        let key = VirtualItemKey::new(8).unwrap();
        let keys = [key, key];
        let build = |_| {
            Some(Widget::Button {
                text: "Open",
                on_press: (),
                style: Style::default(),
                color: None,
                variant: Default::default(),
            })
        };
        let result = KeyedVirtualItems::try_new(&keys, &build);

        assert!(matches!(
            result,
            Err(KeyedVirtualItemsError::DuplicateKey {
                key: duplicate_key,
                first_index: 0,
                duplicate_index: 1,
            }) if duplicate_key == key
        ));
    }
}

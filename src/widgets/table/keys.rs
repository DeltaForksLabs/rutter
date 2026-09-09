// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

/// Stable application identity for one table row.
///
/// ```rust
/// use rutter::table::TableRowKey;
/// let key = TableRowKey::new(42);
/// assert_eq!(key.get(), 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableRowKey(u64);

impl TableRowKey {
    /// Creates a stable row key from an application-owned integer.
    ///
    /// ```rust
    /// use rutter::table::TableRowKey;
    /// assert_eq!(TableRowKey::new(7).get(), 7);
    /// ```
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the application-owned integer represented by this key.
    ///
    /// ```rust
    /// use rutter::table::TableRowKey;
    /// assert_eq!(TableRowKey::new(9).get(), 9);
    /// ```
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable application identity for one table column.
///
/// ```rust
/// use rutter::table::TableColumnKey;
/// let key = TableColumnKey::new(3);
/// assert_eq!(key.get(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableColumnKey(u64);

impl TableColumnKey {
    /// Creates a stable column key from an application-owned integer.
    ///
    /// ```rust
    /// use rutter::table::TableColumnKey;
    /// assert_eq!(TableColumnKey::new(5).get(), 5);
    /// ```
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the application-owned integer represented by this key.
    ///
    /// ```rust
    /// use rutter::table::TableColumnKey;
    /// assert_eq!(TableColumnKey::new(11).get(), 11);
    /// ```
    pub const fn get(self) -> u64 {
        self.0
    }
}

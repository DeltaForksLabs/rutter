// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::{HashMap, hash_map::Entry};

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};

const DEFAULT_TEXT_SHAPE_CACHE_ENTRIES: usize = 128;
const DEFAULT_TEXT_SHAPE_CACHE_ENTRY_BYTES: usize = 16 * 1024;
const DEFAULT_TEXT_SHAPE_CACHE_TOTAL_TEXT_BYTES: usize = 512 * 1024;
const HARD_MAX_TEXT_SHAPE_CACHE_ENTRIES: usize = 256;
const HARD_MAX_TEXT_SHAPE_CACHE_ENTRY_BYTES: usize = 64 * 1024;
const HARD_MAX_TEXT_SHAPE_CACHE_TOTAL_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// Bounds source text retained by [`TextBufferCache`].
///
/// Each cached shape is charged conservatively for the `String` in its key
/// and the text copied into its [`Buffer`], or at least `2 * text.len()` bytes.
/// The total budget applies only to persistent cache entries. Use
/// [`Self::with_shaped`] to drop oversized transient buffers after rendering.
/// Defaults allow 128 entries, 16 KiB per source text, and 512 KiB total.
///
/// # Examples
///
/// ```
/// use rutter::render::text::{TextBufferCache, TextShapeCacheLimits};
///
/// let limits = TextShapeCacheLimits {
///     max_entries: 32,
///     max_entry_bytes: 8 * 1024,
///     max_total_text_bytes: 128 * 1024,
/// };
/// let cache = TextBufferCache::with_limits(limits);
/// assert_eq!(cache.len(), 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextShapeCacheLimits {
    /// Maximum number of persistent shaped entries.
    pub max_entries: usize,
    /// Maximum source text length accepted for one persistent entry.
    pub max_entry_bytes: usize,
    /// Maximum conservative source-text bytes retained by persistent entries.
    pub max_total_text_bytes: usize,
}

impl Default for TextShapeCacheLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_TEXT_SHAPE_CACHE_ENTRIES,
            max_entry_bytes: DEFAULT_TEXT_SHAPE_CACHE_ENTRY_BYTES,
            max_total_text_bytes: DEFAULT_TEXT_SHAPE_CACHE_TOTAL_TEXT_BYTES,
        }
    }
}

impl TextShapeCacheLimits {
    /// Restricts application-provided cache budgets to framework hard caps.
    ///
    /// ```
    /// use rutter::TextShapeCacheLimits;
    ///
    /// let limits = TextShapeCacheLimits { max_entries: usize::MAX, ..Default::default() };
    /// assert_eq!(limits.clamp_to_hard_caps().max_entries, 256);
    /// ```
    pub const fn clamp_to_hard_caps(self) -> Self {
        Self {
            max_entries: cap_text_cache_limit(self.max_entries, HARD_MAX_TEXT_SHAPE_CACHE_ENTRIES),
            max_entry_bytes: cap_text_cache_limit(
                self.max_entry_bytes,
                HARD_MAX_TEXT_SHAPE_CACHE_ENTRY_BYTES,
            ),
            max_total_text_bytes: cap_text_cache_limit(
                self.max_total_text_bytes,
                HARD_MAX_TEXT_SHAPE_CACHE_TOTAL_TEXT_BYTES,
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct TextShapeKey {
    text: String,
    font_size_bits: u32,
    line_height_bits: u32,
    width_bits: Option<u32>,
    height_bits: Option<u32>,
    wrap: u8,
}

impl TextShapeKey {
    fn from_request(request: &TextShapeRequest<'_>) -> Self {
        Self {
            text: request.text.to_string(),
            font_size_bits: request.font_size.to_bits(),
            line_height_bits: request.line_height.to_bits(),
            width_bits: request.width.map(f32::to_bits),
            height_bits: request.height.map(f32::to_bits),
            wrap: wrap_key(request.wrap),
        }
    }
}

#[derive(Debug)]
struct TextShapeCacheEntry {
    buffer: Buffer,
    retained_text_bytes: usize,
    last_access: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TextShapeRequest<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub line_height: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub wrap: Wrap,
}

impl<'a> TextShapeRequest<'a> {
    pub fn new(text: &'a str, font_size: f32, line_height: f32) -> Self {
        Self {
            text,
            font_size,
            line_height,
            width: None,
            height: None,
            wrap: Wrap::WordOrGlyph,
        }
    }

    pub fn with_bounds(mut self, width: Option<f32>, height: Option<f32>) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }
}

#[derive(Debug)]
pub struct TextBufferCache {
    entries: HashMap<TextShapeKey, TextShapeCacheEntry>,
    scratch: Option<Buffer>,
    limits: TextShapeCacheLimits,
    cached_text_bytes: usize,
    scratch_text_bytes: usize,
    access_counter: u64,
}

impl Default for TextBufferCache {
    fn default() -> Self {
        Self::with_limits(TextShapeCacheLimits::default())
    }
}

impl TextBufferCache {
    /// Creates a cache with the default text-byte limits and `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        let limits = TextShapeCacheLimits {
            max_entries: capacity,
            ..TextShapeCacheLimits::default()
        };
        Self::with_limits(limits)
    }

    /// Creates a cache with explicit entry and source-text retention limits.
    pub fn with_limits(limits: TextShapeCacheLimits) -> Self {
        Self {
            entries: HashMap::new(),
            scratch: None,
            limits: limits.clamp_to_hard_caps(),
            cached_text_bytes: 0,
            scratch_text_bytes: 0,
            access_counter: 0,
        }
    }

    /// Returns the number of persistent shaped entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the effective cache limits after hard-cap clamping.
    ///
    /// ```
    /// use rutter::render::text::TextBufferCache;
    ///
    /// assert!(TextBufferCache::default().limits().max_entries > 0);
    /// ```
    pub fn limits(&self) -> TextShapeCacheLimits {
        self.limits
    }

    /// Returns whether a request is currently cached without changing its LRU age.
    pub fn contains(&self, request: TextShapeRequest<'_>) -> bool {
        if self.cacheable_entry_bytes(request.text).is_none() {
            return false;
        }
        self.entries
            .contains_key(&TextShapeKey::from_request(&request))
    }

    /// Returns the conservative source-text byte estimate for persistent entries.
    ///
    /// Each entry includes at least the key `String` and its `Buffer` text,
    /// so this charges at least `2 * text.len()` bytes per cached request.
    pub fn retained_text_bytes(&self) -> usize {
        self.cached_text_bytes
    }

    /// Returns the source-text byte estimate held by the transient scratch buffer.
    pub fn scratch_text_bytes(&self) -> usize {
        self.scratch_text_bytes
    }

    /// Shapes text for one operation without retaining bypassed buffers.
    ///
    /// Use this for rendering so text over the persistent-cache budget is
    /// released as soon as `consume` returns.
    ///
    /// ```
    /// use rutter::cosmic_text::FontSystem;
    /// use rutter::render::text::{TextBufferCache, TextShapeRequest};
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut cache = TextBufferCache::default();
    /// let line_count = cache.with_shaped(
    ///     &mut fonts,
    ///     TextShapeRequest::new("hello", 14.0, 18.0),
    ///     |buffer, _| buffer.layout_runs().count(),
    /// );
    /// assert!(line_count > 0);
    /// ```
    pub fn with_shaped<T>(
        &mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
        consume: impl FnOnce(&Buffer, &mut FontSystem) -> T,
    ) -> T {
        self.clear_scratch();
        let Some(entry_bytes) = self.cacheable_entry_bytes(request.text) else {
            return consume_uncached_shape(fs, request, consume);
        };
        self.consume_cached_shape(fs, request, entry_bytes, consume)
    }

    fn consume_cached_shape<T>(
        &mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
        entry_bytes: usize,
        consume: impl FnOnce(&Buffer, &mut FontSystem) -> T,
    ) -> T {
        let key = TextShapeKey::from_request(&request);
        if self.entries.contains_key(&key) {
            self.touch(&key);
            return consume(self.cached_buffer(&key), fs);
        }
        self.evict_until_room_for(entry_bytes);
        consume(self.insert(fs, key, request, entry_bytes), fs)
    }

    /// Releases the transient buffer retained by [`Self::get_or_shape`].
    ///
    /// New rendering code should prefer [`Self::with_shaped`], which releases
    /// a bypassed buffer automatically after its callback returns.
    pub fn clear_transient_buffer(&mut self) {
        self.clear_scratch();
    }

    pub fn get_or_shape<'a>(
        &'a mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
    ) -> &'a Buffer {
        let Some(retained_text_bytes) = self.cacheable_entry_bytes(request.text) else {
            return self.shape_in_scratch(fs, request);
        };
        self.get_cached_shape(fs, request, retained_text_bytes)
    }

    fn get_cached_shape<'a>(
        &'a mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
        retained_text_bytes: usize,
    ) -> &'a Buffer {
        let key = TextShapeKey::from_request(&request);
        if self.entries.contains_key(&key) {
            self.clear_scratch();
            self.touch(&key);
            return self.cached_buffer(&key);
        }
        self.clear_scratch();
        self.evict_until_room_for(retained_text_bytes);
        self.insert(fs, key, request, retained_text_bytes)
    }

    fn cached_buffer(&self, key: &TextShapeKey) -> &Buffer {
        &self
            .entries
            .get(key)
            .expect("text buffer cache hit missing")
            .buffer
    }

    fn touch(&mut self, key: &TextShapeKey) {
        let last_access = self.next_access();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = last_access;
        }
    }

    fn cacheable_entry_bytes(&self, text: &str) -> Option<usize> {
        if self.limits.max_entries == 0 || text.len() > self.limits.max_entry_bytes {
            return None;
        }

        let retained_text_bytes = text.len().checked_mul(2)?;
        (retained_text_bytes <= self.limits.max_total_text_bytes).then_some(retained_text_bytes)
    }

    fn evict_until_room_for(&mut self, entry_bytes: usize) {
        while !self.has_room_for(entry_bytes) {
            if !self.evict_least_recently_used() {
                self.cached_text_bytes = 0;
                break;
            }
        }
    }

    fn has_room_for(&self, entry_bytes: usize) -> bool {
        if self.entries.len() >= self.limits.max_entries {
            return false;
        }

        self.cached_text_bytes
            .checked_add(entry_bytes)
            .is_some_and(|total| total <= self.limits.max_total_text_bytes)
    }

    fn evict_least_recently_used(&mut self) -> bool {
        let Some(oldest_access) = self.entries.values().map(|entry| entry.last_access).min() else {
            return false;
        };
        let mut evicted_bytes = None;
        self.entries.retain(|_, entry| {
            if entry.last_access != oldest_access || evicted_bytes.is_some() {
                return true;
            }

            evicted_bytes = Some(entry.retained_text_bytes);
            false
        });
        let Some(evicted_bytes) = evicted_bytes else {
            return false;
        };

        self.cached_text_bytes = self.cached_text_bytes.saturating_sub(evicted_bytes);
        true
    }

    fn insert(
        &mut self,
        fs: &mut FontSystem,
        key: TextShapeKey,
        request: TextShapeRequest<'_>,
        retained_text_bytes: usize,
    ) -> &Buffer {
        debug_assert!(self.has_room_for(retained_text_bytes));
        let buffer = shape_text_buffer(fs, request);
        let entry = TextShapeCacheEntry {
            buffer,
            retained_text_bytes,
            last_access: self.next_access(),
        };
        self.cached_text_bytes = self.cached_text_bytes.saturating_add(retained_text_bytes);
        match self.entries.entry(key) {
            Entry::Vacant(slot) => &slot.insert(entry).buffer,
            Entry::Occupied(_) => unreachable!("text buffer cache entry inserted after cache miss"),
        }
    }

    fn shape_in_scratch<'a>(
        &'a mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
    ) -> &'a Buffer {
        self.clear_scratch();
        let scratch_text_bytes = request.text.len();
        self.scratch = Some(shape_text_buffer(fs, request));
        self.scratch_text_bytes = scratch_text_bytes;
        self.scratch
            .as_ref()
            .expect("text buffer scratch missing after shape")
    }

    fn clear_scratch(&mut self) {
        self.scratch = None;
        self.scratch_text_bytes = 0;
    }

    fn next_access(&mut self) -> u64 {
        self.access_counter = self.access_counter.saturating_add(1);
        self.access_counter
    }
}

fn consume_uncached_shape<T>(
    fs: &mut FontSystem,
    request: TextShapeRequest<'_>,
    consume: impl FnOnce(&Buffer, &mut FontSystem) -> T,
) -> T {
    let buffer = shape_text_buffer(fs, request);
    consume(&buffer, fs)
}

fn shape_text_buffer(fs: &mut FontSystem, request: TextShapeRequest<'_>) -> Buffer {
    let mut buffer = Buffer::new(fs, Metrics::new(request.font_size, request.line_height));
    buffer.set_wrap(fs, request.wrap);
    buffer.set_size(fs, request.width, request.height);
    buffer.set_text(fs, request.text, &Attrs::new(), Shaping::Advanced, None);
    buffer
}

const fn wrap_key(wrap: Wrap) -> u8 {
    match wrap {
        Wrap::None => 0,
        Wrap::Glyph => 1,
        Wrap::Word => 2,
        Wrap::WordOrGlyph => 3,
    }
}

const fn cap_text_cache_limit(value: usize, maximum: usize) -> usize {
    if value > maximum { maximum } else { value }
}

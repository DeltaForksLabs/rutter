// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    hash::Hash,
    rc::Rc,
};

use cosmic_text::FontSystem;
use skia_safe::Image;

use super::image::RutterDecodedImage;
use super::rich_text::RichTextRenderer;

const MAX_RASTER_CACHE_BYTES: usize = 128 * 1024 * 1024;
const MAX_SVG_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SvgImageCacheKey {
    pub data_hash: u64,
    pub width: u32,
    pub height: u32,
    pub scale_bits: u32,
}

pub struct ImageRenderCache {
    raster_images: MemoryBoundedLru<u64, RutterDecodedImage>,
    svg_images: MemoryBoundedLru<SvgImageCacheKey, Image>,
    layout_font_system: Rc<RefCell<FontSystem>>,
    rich_text_renderer: RichTextRenderer,
}

impl Default for ImageRenderCache {
    fn default() -> Self {
        Self {
            raster_images: MemoryBoundedLru::new(MAX_RASTER_CACHE_BYTES),
            svg_images: MemoryBoundedLru::new(MAX_SVG_CACHE_BYTES),
            layout_font_system: Rc::new(RefCell::new(FontSystem::new())),
            rich_text_renderer: RichTextRenderer::new(),
        }
    }
}

impl ImageRenderCache {
    /// Clears decoded image and rasterized SVG entries retained by the renderer.
    ///
    /// Example:
    ///
    /// ```rust
    /// let mut cache = rutter::render::ImageRenderCache::default();
    /// cache.clear();
    /// ```
    pub fn clear(&mut self) {
        self.raster_images.clear();
        self.svg_images.clear();
        self.rich_text_renderer.clear();
    }

    pub(crate) fn raster_image(&mut self, key: u64) -> Option<RutterDecodedImage> {
        self.raster_images.get(&key)
    }

    pub(crate) fn insert_raster_image(&mut self, key: u64, decoded: RutterDecodedImage) {
        let estimated_bytes = decoded_image_bytes(&decoded);
        self.raster_images.insert(key, decoded, estimated_bytes);
    }

    pub(crate) fn svg_image(&mut self, key: SvgImageCacheKey) -> Option<Image> {
        self.svg_images.get(&key)
    }

    pub(crate) fn insert_svg_image(&mut self, key: SvgImageCacheKey, image: Image) {
        self.svg_images
            .insert(key, image.clone(), skia_image_bytes(&image));
    }

    pub(crate) fn layout_font_system(&self) -> Rc<RefCell<FontSystem>> {
        self.layout_font_system.clone()
    }

    pub(crate) fn rich_text_renderer(&self) -> &RichTextRenderer {
        &self.rich_text_renderer
    }
}

struct MemoryBoundedLru<K, V> {
    entries: HashMap<K, CachedImage<V>>,
    least_recent_keys: VecDeque<K>,
    used_bytes: usize,
    max_bytes: usize,
}

struct CachedImage<V> {
    value: V,
    estimated_bytes: usize,
}

impl<K, V> MemoryBoundedLru<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            least_recent_keys: VecDeque::new(),
            used_bytes: 0,
            max_bytes,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        let value = self.entries.get(key)?.value.clone();
        self.promote(key);
        Some(value)
    }

    fn insert(&mut self, key: K, value: V, estimated_bytes: usize) {
        if estimated_bytes > self.max_bytes {
            return;
        }
        self.remove_existing(&key);
        self.evict_to_fit(estimated_bytes);
        self.used_bytes += estimated_bytes;
        self.least_recent_keys.push_back(key.clone());
        self.entries.insert(
            key,
            CachedImage {
                value,
                estimated_bytes,
            },
        );
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.least_recent_keys.clear();
        self.used_bytes = 0;
    }

    fn promote(&mut self, key: &K) {
        self.least_recent_keys.retain(|candidate| candidate != key);
        self.least_recent_keys.push_back(key.clone());
    }

    fn remove_existing(&mut self, key: &K) {
        let Some(previous) = self.entries.remove(key) else {
            return;
        };
        self.used_bytes = self.used_bytes.saturating_sub(previous.estimated_bytes);
        self.least_recent_keys.retain(|candidate| candidate != key);
    }

    fn evict_to_fit(&mut self, incoming_bytes: usize) {
        while self.used_bytes.saturating_add(incoming_bytes) > self.max_bytes {
            let Some(key) = self.least_recent_keys.pop_front() else {
                return;
            };
            self.remove_existing(&key);
        }
    }
}

fn decoded_image_bytes(image: &RutterDecodedImage) -> usize {
    dimensions_byte_cost(image.width, image.height)
}

fn skia_image_bytes(image: &Image) -> usize {
    dimensions_byte_cost(image.width(), image.height())
}

fn dimensions_byte_cost(width: i32, height: i32) -> usize {
    usize::try_from(width.max(0))
        .ok()
        .and_then(|width| {
            usize::try_from(height.max(0))
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::MemoryBoundedLru;

    #[test]
    fn memory_bounded_lru_evicts_only_least_recent_entry() {
        let mut cache = MemoryBoundedLru::new(8);
        cache.insert("first", 1, 4);
        cache.insert("second", 2, 4);
        assert_eq!(cache.get(&"first"), Some(1));

        cache.insert("third", 3, 4);

        assert_eq!(cache.get(&"first"), Some(1));
        assert_eq!(cache.get(&"second"), None);
        assert_eq!(cache.get(&"third"), Some(3));
    }

    #[test]
    fn memory_bounded_lru_does_not_retain_oversized_entry() {
        let mut cache = MemoryBoundedLru::new(4);
        cache.insert("large", 1, 5);

        assert_eq!(cache.get(&"large"), None);
        assert_eq!(cache.used_bytes, 0);
    }
}

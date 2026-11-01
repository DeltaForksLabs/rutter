//! Opt-in constructor profiles, not latency tests or performance gates.
//!
//! Run each probe in fresh processes with `--release --ignored --nocapture
//! --test-threads=1`. The cache's first sample precedes explicit font discovery
//! in that process (not OS-cache-cold); component probes and the shared-font
//! engine already follow font initialization. Warm samples follow three
//! untimed constructions. Construction is timed,
//! destruction is outside the interval. No native surface is created here.

use std::{cell::RefCell, hint::black_box, rc::Rc, time::Instant};

use cosmic_text::FontSystem;
use skia_safe::Image;

use super::{
    ImageRenderCache, MAX_RASTER_CACHE_BYTES, MAX_SVG_CACHE_BYTES, MemoryBoundedLru,
    RichTextRenderer, RutterDecodedImage, SvgImageCacheKey,
};
use crate::{AppLogic, Widget, engine::RutterEngine};

fn constructor_samples<T>(name: &str, mut construct: impl FnMut() -> T) {
    let started = Instant::now();
    let cold = black_box(construct());
    let cold_ns = started.elapsed().as_nanos();
    drop(cold);
    for _ in 0..3 {
        drop(black_box(construct()));
    }
    let mut samples = Vec::with_capacity(32);
    for _ in 0..32 {
        let started = Instant::now();
        let resource = black_box(construct());
        samples.push(started.elapsed().as_nanos());
        drop(resource);
    }
    samples.sort_unstable();
    println!(
        "{name}: first={cold_ns}ns warm_samples=32 min={}ns median={}ns p95={}ns max={}ns",
        samples[0], samples[16], samples[30], samples[31]
    );
}

#[test]
#[ignore = "manual constructor profile; no display required or timing assertions"]
fn image_cache_constructor_profile() {
    // Profile the complete cache first, before explicit font discovery. Each
    // component's first sample below is first-use, not a fresh process sample.
    constructor_samples("image_cache", ImageRenderCache::default);
    constructor_samples("font_system", FontSystem::new);
    constructor_samples("rich_text_renderer", RichTextRenderer::new);
    constructor_samples("empty_image_lrus", || {
        (
            MemoryBoundedLru::<u64, RutterDecodedImage>::new(MAX_RASTER_CACHE_BYTES),
            MemoryBoundedLru::<SvgImageCacheKey, Image>::new(MAX_SVG_CACHE_BYTES),
        )
    });
}

struct EmptySurface;

impl AppLogic for EmptySurface {
    type State = ();
    type Message = ();

    fn new(_: &mut FontSystem) {}

    fn view(_: &mut ()) -> Widget<'_, ()> {
        Widget::Spacer {
            style: Default::default(),
        }
    }

    fn update(_: &mut (), _: (), _: &mut arboard::Clipboard) {}
}

#[test]
#[ignore = "manual shared-font engine profile; requires an isolated clipboard/display"]
fn shared_font_surface_constructor_profile() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    constructor_samples("shared_font_engine_no_native_surface", || {
        RutterEngine::<EmptySurface>::with_shared_font_system((), fonts.clone(), Default::default())
            .expect("constructor profile requires a usable isolated clipboard/display")
    });
}

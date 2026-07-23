use rutter::cosmic_text::{Buffer, FontSystem};
use rutter::render::text::{TextBufferCache, TextShapeCacheLimits, TextShapeRequest};

fn cache_limits(
    max_entries: usize,
    max_entry_bytes: usize,
    max_total_text_bytes: usize,
) -> TextShapeCacheLimits {
    TextShapeCacheLimits {
        max_entries,
        max_entry_bytes,
        max_total_text_bytes,
    }
}

fn shape_request(text: &str) -> TextShapeRequest<'_> {
    TextShapeRequest::new(text, 14.0, 18.0)
}

#[test]
fn oversized_render_request_drops_its_transient_buffer() {
    let oversized = "oversized";
    let mut cache = TextBufferCache::with_limits(cache_limits(4, 4, 64));
    let mut fonts = FontSystem::new();

    let line_count = cache.with_shaped(&mut fonts, shape_request(oversized), |buffer, _| {
        buffer.layout_runs().count()
    });

    assert_eq!(cache.len(), 0);
    assert_eq!(cache.retained_text_bytes(), 0);
    assert_eq!(cache.scratch_text_bytes(), 0);
    assert!(line_count > 0);
}

#[test]
fn total_text_budget_evicts_the_least_recently_used_entry() {
    let mut cache = TextBufferCache::with_limits(cache_limits(4, 4, 16));
    let mut fonts = FontSystem::new();
    let alpha = shape_request("aaaa");
    let beta = shape_request("bbbb");
    let gamma = shape_request("cccc");

    let _ = cache.get_or_shape(&mut fonts, alpha);
    let _ = cache.get_or_shape(&mut fonts, beta);
    let _ = cache.get_or_shape(&mut fonts, beta);
    let _ = cache.get_or_shape(&mut fonts, gamma);

    assert!(!cache.contains(alpha));
    assert!(cache.contains(beta));
    assert!(cache.contains(gamma));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.retained_text_bytes(), 16);
}

#[test]
fn cache_hit_does_not_increase_retained_text_bytes() {
    let mut cache = TextBufferCache::with_limits(cache_limits(2, 16, 64));
    let mut fonts = FontSystem::new();
    let request = shape_request("cached");

    let first = cache.get_or_shape(&mut fonts, request) as *const Buffer;
    let retained_after_first = cache.retained_text_bytes();
    let second = cache.get_or_shape(&mut fonts, request) as *const Buffer;

    assert_eq!(first, second);
    assert_eq!(cache.retained_text_bytes(), retained_after_first);
}

#[test]
fn cache_respects_the_maximum_entry_count() {
    let mut cache = TextBufferCache::with_limits(cache_limits(2, 4, 64));
    let mut fonts = FontSystem::new();
    let alpha = shape_request("a");
    let beta = shape_request("b");
    let gamma = shape_request("c");

    let _ = cache.get_or_shape(&mut fonts, alpha);
    let _ = cache.get_or_shape(&mut fonts, beta);
    let _ = cache.get_or_shape(&mut fonts, gamma);

    assert_eq!(cache.len(), 2);
    assert!(!cache.contains(alpha));
    assert!(cache.contains(beta));
    assert!(cache.contains(gamma));
}

#[test]
fn zero_entry_cache_does_not_retain_a_rendered_buffer() {
    let mut cache = TextBufferCache::new(0);
    let mut fonts = FontSystem::new();

    cache.with_shaped(&mut fonts, shape_request("uncached"), |buffer, _| {
        assert!(buffer.layout_runs().count() > 0);
    });

    assert_eq!(cache.scratch_text_bytes(), 0);
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.retained_text_bytes(), 0);
}

#[test]
fn cache_limits_cannot_exceed_framework_hard_caps() {
    let cache = TextBufferCache::with_limits(cache_limits(usize::MAX, usize::MAX, usize::MAX));

    assert_eq!(cache.limits().max_entries, 256);
    assert_eq!(cache.limits().max_entry_bytes, 64 * 1024);
    assert_eq!(cache.limits().max_total_text_bytes, 2 * 1024 * 1024);
}

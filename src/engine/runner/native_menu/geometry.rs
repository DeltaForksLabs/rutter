// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Pure native menu coordinates, anchors and drawable validation.

use crate::engine::gpu::validate_native_popup_size;
use crate::render::NativeMenuKind;
use crate::widgets::dropdown_menu::DropdownMenuSurface;
use skia_safe::{Point, Rect};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::window::{
    PopupSurfaceGeometry, WindowAnchor, WindowAttributes, WindowConstraintAdjustment,
    WindowGravity, WindowPositioner, WindowType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MenuPlacement {
    pub(super) kind: NativeMenuKind,
    pub(super) position: PhysicalPosition<i32>,
    pub(super) size: PhysicalSize<u32>,
}

pub(super) fn menu_surface_union(surfaces: &[DropdownMenuSurface]) -> Option<Rect> {
    let first = surfaces.first()?;
    Some(surfaces.iter().skip(1).fold(first.rect, |union, panel| {
        Rect::from_ltrb(
            union.left.min(panel.rect.left),
            union.top.min(panel.rect.top),
            union.right.max(panel.rect.right),
            union.bottom.max(panel.rect.bottom),
        )
    }))
}

pub(super) fn physical_menu_rect(
    kind: NativeMenuKind,
    rect: Rect,
    scale: f32,
) -> Result<MenuPlacement, String> {
    if !scale.is_finite()
        || scale <= 0.0
        || [rect.left, rect.top, rect.right, rect.bottom]
            .iter()
            .any(|edge| !edge.is_finite())
    {
        return Err("native menu requires finite coordinates and a positive finite scale".into());
    }
    // Use f64 so the i32 upper bound and edge differences remain exact.
    let left = (f64::from(rect.left) * f64::from(scale)).floor();
    let top = (f64::from(rect.top) * f64::from(scale)).floor();
    let right = (f64::from(rect.right) * f64::from(scale)).ceil();
    let bottom = (f64::from(rect.bottom) * f64::from(scale)).ceil();
    if left < f64::from(i32::MIN)
        || top < f64::from(i32::MIN)
        || right > f64::from(i32::MAX)
        || bottom > f64::from(i32::MAX)
        || right <= left
        || bottom <= top
    {
        return Err("native menu physical edges are outside supported coordinates".into());
    }
    let size = PhysicalSize::new((right - left) as u32, (bottom - top) as u32);
    validate_native_popup_size(size)?;
    Ok(MenuPlacement {
        kind,
        position: PhysicalPosition::new(left as i32, top as i32),
        size,
    })
}

pub(super) fn popup_logical_pointer(
    local: PhysicalPosition<f64>,
    geometry: PopupSurfaceGeometry,
) -> Point {
    Point::new(
        (geometry.parent_position.x + local.x / geometry.scale_factor) as f32,
        (geometry.parent_position.y + local.y / geometry.scale_factor) as f32,
    )
}

pub(super) fn configured_rect(geometry: PopupSurfaceGeometry) -> Result<Rect, String> {
    validate_native_popup_size(geometry.surface_size)?;
    if !geometry.scale_factor.is_finite()
        || geometry.scale_factor <= 0.0
        || !geometry.parent_position.x.is_finite()
        || !geometry.parent_position.y.is_finite()
    {
        return Err(
            "native popup configure requires a finite position and positive finite scale".into(),
        );
    }
    Ok(Rect::from_xywh(
        geometry.parent_position.x as f32,
        geometry.parent_position.y as f32,
        (f64::from(geometry.surface_size.width) / geometry.scale_factor) as f32,
        (f64::from(geometry.surface_size.height) / geometry.scale_factor) as f32,
    ))
}

pub(super) fn same_drawable(left: PopupSurfaceGeometry, right: PopupSurfaceGeometry) -> bool {
    left.parent_position == right.parent_position
        && left.surface_size == right.surface_size
        && left.scale_factor == right.scale_factor
}

pub(super) fn menu_positioner(
    position: PhysicalPosition<i32>,
    parent: PhysicalSize<u32>,
    scale: f32,
) -> WindowPositioner {
    let origin = Point::new(position.x as f32 / scale, position.y as f32 / scale);
    let anchor = Point::new(
        origin
            .x
            .clamp(0.0, (parent.width as f32 / scale - 1.0).max(0.0)),
        origin
            .y
            .clamp(0.0, (parent.height as f32 / scale - 1.0).max(0.0)),
    );
    WindowPositioner::new(
        WindowAnchor::TopLeft,
        (
            LogicalPosition::new(anchor.x, anchor.y).into(),
            LogicalSize::new(1.0, 1.0).into(),
        ),
        LogicalPosition::new(origin.x - anchor.x, origin.y - anchor.y).into(),
        WindowGravity::BottomRight,
        WindowConstraintAdjustment::all(),
    )
}

pub(super) fn native_menu_attributes(
    placement: MenuPlacement,
    parent: PhysicalSize<u32>,
    scale: f32,
) -> WindowAttributes {
    WindowAttributes::default()
        .with_window_type(WindowType::Popup)
        .with_decorations(false)
        // Union gaps and rounded corners must expose the surfaces underneath.
        .with_transparent(true)
        .with_active(true)
        .with_positioner(menu_positioner(placement.position, parent, scale))
        .with_surface_size(placement.size.to_logical::<f64>(f64::from(scale)))
        .with_visible(true)
}

pub(super) fn usable_menu_area(size: PhysicalSize<u32>, scale: f32, kind: NativeMenuKind) -> bool {
    scale.is_finite()
        && scale > 0.0
        && size.width as f32 / scale >= 64.0
        && size.height as f32 / scale
            >= match kind {
                NativeMenuKind::Context(_) => 44.0,
                NativeMenuKind::Dropdown(_) => 40.0,
            }
}

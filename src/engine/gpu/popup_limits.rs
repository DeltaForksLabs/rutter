// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Native popup drawable limits, enforced before graphics allocation.

use winit::{dpi::PhysicalSize, window::WindowType};

pub(crate) const MAX_NATIVE_POPUP_DIMENSION: u32 = 4096;
pub(crate) const MAX_NATIVE_POPUP_PIXELS: u64 = 4 * 1024 * 1024;

pub(crate) fn validate_native_popup_size(size: PhysicalSize<u32>) -> Result<(), String> {
    let pixels = u64::from(size.width) * u64::from(size.height);
    if size.width == 0
        || size.height == 0
        || size.width > MAX_NATIVE_POPUP_DIMENSION
        || size.height > MAX_NATIVE_POPUP_DIMENSION
        || pixels > MAX_NATIVE_POPUP_PIXELS
    {
        return Err(format!(
            "native popup drawable {}x{} exceeds its allocation contract: expected positive dimensions <= {MAX_NATIVE_POPUP_DIMENSION} and <= {MAX_NATIVE_POPUP_PIXELS} physical pixels per buffer",
            size.width, size.height,
        ));
    }
    Ok(())
}

pub(super) fn validate_popup_allocation(
    kind: WindowType,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    if kind == WindowType::Popup {
        validate_native_popup_size(size)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_popup_allocation_budget_checks_area_axes_zero_and_integer_extremes() {
        assert!(validate_native_popup_size(PhysicalSize::new(4096, 1024)).is_ok());
        assert!(validate_native_popup_size(PhysicalSize::new(2048, 2048)).is_ok());
        for size in [
            PhysicalSize::new(4096, 1025),
            PhysicalSize::new(4097, 1),
            PhysicalSize::new(1, 4097),
            PhysicalSize::new(u32::MAX, u32::MAX),
            PhysicalSize::new(0, 44),
        ] {
            assert!(validate_popup_allocation(WindowType::Popup, size).is_err());
        }
        // This change must not impose menu policy on ordinary application windows.
        assert!(
            validate_popup_allocation(WindowType::Window, PhysicalSize::new(7680, 4320)).is_ok()
        );
    }
}

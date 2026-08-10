// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

mod geometry;
mod model;
mod runtime;
mod state;

pub use model::{DropdownMenuEntry, DropdownMenuEntryKind};
pub use state::DropdownMenuState;

#[allow(unused_imports)]
pub(crate) use geometry::{
    DropdownMenuSurface, ITEM_ROW_HEIGHT, MAX_HEIGHT, MENU_PADDING, MIN_WIDTH, ROOT_GAP,
    SEPARATOR_HEIGHT, SUBMENU_OVERLAP, VIEWPORT_MARGIN, build_open_menu_surfaces, clamp_scroll,
    estimate_content_height, estimate_level_width, maximum_scroll, place_root_surface,
    place_submenu_surface, point_to_entry, row_rect, scroll_to_reveal,
};
#[allow(unused_imports)]
pub(crate) use runtime::{
    DropdownMenuEntryAccess, OwnedDropdownMenuEntry, entries_at_level, entry_at_path,
    first_focusable_index, flatten_entry_paths, last_focusable_index, next_focusable_index,
    previous_focusable_index, to_owned_entries, typeahead_prefix_match,
};

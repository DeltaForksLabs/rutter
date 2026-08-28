// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — lib.rs
// Ponto de entrada público. Declare módulos e re-exporte o
// que o usuário do framework precisa importar.
// ============================================================

pub(crate) mod accessibility;
pub mod app;
pub mod engine;
pub mod i18n;
mod input;
pub mod layout;
pub mod multi_window;
pub mod render;
pub mod theme;
pub mod widget;
mod widgets;

// Keep established public paths while the implementation stays grouped by input domain.
pub use input::{limits as input_limits, state as input_state};

// ── Re-exports ergonômicos ───────────────────────────────────
pub use app::{
    AppLogic, LogicalPointerPosition, PhysicalDesktopPosition, SecondaryPointerContext,
    SurfaceConfig,
};
pub use calendar::{
    CalendarConfig, CalendarDate, CalendarError, CalendarLabels, CalendarMonth, WeekStart,
};
pub use carousel::{CarouselConfig, CarouselConfigError};
pub use dropdown_menu::{DropdownMenuEntry, DropdownMenuEntryKind};
pub use engine::multi_runner::MultiWindowRunner;
pub use engine::run_error::RutterRunError;
pub use engine::runner::RutterRunner;
pub use i18n::{FluentCatalog, I18nError, LayoutDirection, Locale};
pub use input_limits::{InputKind, InputLimitError, InputLimits};
pub use multi_window::{
    CloseBehavior, MultiWindowAppLogic, MultiWindowRunError, SurfaceCommand, SurfaceEvent,
    SurfaceId, SurfaceRequest, WindowConfig, WindowConfigError, WindowLevel, WindowPosition,
    WindowSize,
};
pub use render::text::TextShapeCacheLimits;
pub use rich_text::{
    RichText, RichTextColor, RichTextError, RichTextSize, RichTextSlant, RichTextSpan,
    RichTextSpanStyle, RichTextStyle, RichTextWeight,
};
pub use theme::Theme;
pub use time::{
    ClockConfig, ClockError, ClockFormat, ClockTime, HourCycle, LocalTimeResolution, TimeOfDay,
    TimeOfDayError, TimePickerConfig, TimePickerError, TimePickerLabels, TimeZone, TimeZoneError,
};
pub use widget::id::{WidgetId, WidgetIdError, WidgetIdSnapshot};
pub use widget::{
    AUTO_ID, ButtonVariant, ContextMenuEntry, CounterConfigError, DialogPosition, InputState,
    VirtualSelection, Widget, WidgetConfigError, validate_counter, validate_slider,
    validate_virtual_grid, validate_virtual_list,
};
pub use widgets::{calendar, carousel, dropdown_menu, rich_text, time};

// ── Re-exports de dependências públicas ──────────────────────
pub use arboard;
pub use chrono_tz;
pub use cosmic_text;
pub use skia_safe;
pub use taffy;

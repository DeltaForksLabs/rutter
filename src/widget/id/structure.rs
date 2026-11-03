// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use crate::widget::{CounterConfig, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WidgetStructureKind {
    Column,
    Row,
    Container,
    Spacer,
    Divider,
    Text,
    Heading,
    Image,
    Button,
    ButtonContent,
    TextInput,
    PasswordInput,
    TextArea,
    SearchBar,
    ComboBox,
    Checkbox,
    Switch,
    Radio,
    Slider,
    Counter,
    // Private numeric-mode fingerprint for strict same-model reconstruction.
    // Both modes share the public Counter ID tag, allowing published transitions.
    CounterFloatMode,
    Clock,
    Select,
    ProgressBar,
    Spinner,
    ScrollView,
    TableOfContents,
    TableOfContentsAccordionCollapsed,
    TableOfContentsAccordionExpanded,
    Table,
    Tooltip,
    AccordionCollapsed,
    AccordionExpanded,
    TabBar,
    ModalHidden,
    ModalVisible,
    DialogHidden,
    DialogVisible,
    Toast,
    ContextMenu,
    PopoverClosed,
    PopoverOpen,
    CarouselView,
    InteractiveCarouselView,
    VirtualList,
    VirtualListContent,
    InteractiveVirtualListContent,
    VirtualListWithSelection,
    VirtualListContentWithSelection,
    VirtualGrid,
    VirtualGridContent,
    InteractiveVirtualGridContent,
    VirtualGridWithSelection,
    VirtualGridContentWithSelection,
    DropdownMenu,
    Custom,
    PointerRegion,
}

impl WidgetStructureKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Column => "Column",
            Self::Row => "Row",
            Self::Container => "Container",
            Self::Spacer => "Spacer",
            Self::Divider => "Divider",
            Self::Text => "Text",
            Self::Heading => "Heading",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::ButtonContent => "ButtonContent",
            Self::TextInput => "TextInput",
            Self::PasswordInput => "PasswordInput",
            Self::TextArea => "TextArea",
            Self::SearchBar => "SearchBar",
            Self::ComboBox => "ComboBox",
            Self::Checkbox => "Checkbox",
            Self::Switch => "Switch",
            Self::Radio => "Radio",
            Self::Slider => "Slider",
            Self::Counter => "Counter",
            Self::CounterFloatMode => "Counter (f64)",
            Self::Clock => "Clock",
            Self::Select => "Select",
            Self::ProgressBar => "ProgressBar",
            Self::Spinner => "Spinner",
            Self::ScrollView => "ScrollView",
            Self::TableOfContents => "TableOfContents",
            Self::TableOfContentsAccordionCollapsed => "TableOfContentsAccordionCollapsed",
            Self::TableOfContentsAccordionExpanded => "TableOfContentsAccordionExpanded",
            Self::Table => "Table",
            Self::Tooltip => "Tooltip",
            Self::AccordionCollapsed => "AccordionCollapsed",
            Self::AccordionExpanded => "AccordionExpanded",
            Self::TabBar => "TabBar",
            Self::ModalHidden => "ModalHidden",
            Self::ModalVisible => "ModalVisible",
            Self::DialogHidden => "DialogHidden",
            Self::DialogVisible => "DialogVisible",
            Self::Toast => "Toast",
            Self::ContextMenu => "ContextMenu",
            Self::PopoverClosed => "PopoverClosed",
            Self::PopoverOpen => "PopoverOpen",
            Self::CarouselView => "CarouselView",
            Self::InteractiveCarouselView => "InteractiveCarouselView",
            Self::VirtualList => "VirtualList",
            Self::VirtualListContent => "VirtualListContent",
            Self::InteractiveVirtualListContent => "InteractiveVirtualListContent",
            Self::VirtualListWithSelection => "VirtualListWithSelection",
            Self::VirtualListContentWithSelection => "VirtualListContentWithSelection",
            Self::VirtualGrid => "VirtualGrid",
            Self::VirtualGridContent => "VirtualGridContent",
            Self::InteractiveVirtualGridContent => "InteractiveVirtualGridContent",
            Self::VirtualGridWithSelection => "VirtualGridWithSelection",
            Self::VirtualGridContentWithSelection => "VirtualGridContentWithSelection",
            Self::DropdownMenu => "DropdownMenu",
            Self::Custom => "Custom",
            Self::PointerRegion => "PointerRegion",
        }
    }
}

pub(crate) fn widget_structure_kind<Msg>(widget: &Widget<'_, Msg>) -> WidgetStructureKind {
    use WidgetStructureKind as Kind;
    match widget {
        Widget::Disabled { child } => widget_structure_kind(child),
        Widget::Column { .. } => Kind::Column,
        Widget::Row { .. } => Kind::Row,
        Widget::Container { .. } => Kind::Container,
        Widget::Spacer { .. } => Kind::Spacer,
        Widget::Divider { .. } => Kind::Divider,
        Widget::Text { .. } | Widget::RichText { .. } => Kind::Text,
        Widget::Heading { .. } => Kind::Heading,
        Widget::Image { .. } => Kind::Image,
        Widget::Button { .. } => Kind::Button,
        Widget::ButtonContent { .. } => Kind::ButtonContent,
        Widget::TextInput {
            is_password: true, ..
        }
        | Widget::TextInputWith {
            is_password: true, ..
        } => Kind::PasswordInput,
        Widget::TextInput { .. } | Widget::TextInputWith { .. } => Kind::TextInput,
        Widget::TextArea { .. } | Widget::TextAreaWith { .. } => Kind::TextArea,
        Widget::SearchBar { .. } | Widget::SearchBarWith { .. } => Kind::SearchBar,
        Widget::ComboBox { .. } | Widget::ComboBoxWith { .. } => Kind::ComboBox,
        Widget::Checkbox { .. } | Widget::CheckboxWith { .. } => Kind::Checkbox,
        Widget::Switch { .. } | Widget::SwitchWith { .. } => Kind::Switch,
        Widget::Radio { .. } | Widget::RadioWith { .. } => Kind::Radio,
        Widget::Slider { .. } | Widget::SliderWith { .. } => Kind::Slider,
        Widget::Counter { config, .. } => match config {
            CounterConfig::Integer(_) | CounterConfig::IntegerWith(_) => Kind::Counter,
            CounterConfig::Float(_) | CounterConfig::FloatWith(_) => Kind::CounterFloatMode,
        },
        Widget::Clock { .. } => Kind::Clock,
        Widget::Select { .. } | Widget::SelectWith { .. } => Kind::Select,
        Widget::ProgressBar { .. } => Kind::ProgressBar,
        Widget::Spinner { .. } => Kind::Spinner,
        Widget::ScrollView { .. } => Kind::ScrollView,
        Widget::TableOfContents { .. } if widget.table_of_contents_accordion().is_some() => {
            if widget.table_of_contents_entries_visible() {
                Kind::TableOfContentsAccordionExpanded
            } else {
                Kind::TableOfContentsAccordionCollapsed
            }
        }
        Widget::TableOfContents { .. } => Kind::TableOfContents,
        Widget::Table { .. } => Kind::Table,
        Widget::Tooltip { .. } => Kind::Tooltip,
        Widget::Accordion { expanded: true, .. } => Kind::AccordionExpanded,
        Widget::Accordion { .. } => Kind::AccordionCollapsed,
        Widget::TabBar { .. } | Widget::TabBarWith { .. } => Kind::TabBar,
        Widget::Modal { visible: true, .. } => Kind::ModalVisible,
        Widget::Modal { .. } => Kind::ModalHidden,
        Widget::Dialog { visible: true, .. } => Kind::DialogVisible,
        Widget::Dialog { .. } => Kind::DialogHidden,
        Widget::Toast { .. } => Kind::Toast,
        Widget::ContextMenu { .. } => Kind::ContextMenu,
        Widget::DropdownMenu { .. } => Kind::DropdownMenu,
        Widget::Custom { .. } => Kind::Custom,
        Widget::PointerRegion { .. } | Widget::PointerRegionWith { .. } => Kind::PointerRegion,
        Widget::Popover { open: true, .. } => Kind::PopoverOpen,
        Widget::Popover { .. } => Kind::PopoverClosed,
        Widget::CarouselView { .. } | Widget::CarouselViewWith { .. } => Kind::CarouselView,
        Widget::InteractiveCarouselView { .. } | Widget::InteractiveCarouselViewWith { .. } => {
            Kind::InteractiveCarouselView
        }
        Widget::VirtualList { .. } => Kind::VirtualList,
        Widget::VirtualListContent { .. } => Kind::VirtualListContent,
        Widget::InteractiveVirtualListContent { .. }
        | Widget::InteractiveVirtualListContentWith { .. } => Kind::InteractiveVirtualListContent,
        Widget::VirtualListWithSelection { .. } => Kind::VirtualListWithSelection,
        Widget::VirtualListContentWithSelection { .. } => Kind::VirtualListContentWithSelection,
        Widget::VirtualListWith { selection, .. } => match selection {
            super::super::VirtualSelectionWith::Single(_) => Kind::VirtualList,
            super::super::VirtualSelectionWith::Multiple { .. } => Kind::VirtualListWithSelection,
        },
        Widget::VirtualListContentWith { selection, .. } => match selection {
            super::super::VirtualSelectionWith::Single(_) => Kind::VirtualListContent,
            super::super::VirtualSelectionWith::Multiple { .. } => {
                Kind::VirtualListContentWithSelection
            }
        },
        Widget::VirtualGrid { .. } => Kind::VirtualGrid,
        Widget::VirtualGridContent { .. } => Kind::VirtualGridContent,
        Widget::InteractiveVirtualGridContent { .. }
        | Widget::InteractiveVirtualGridContentWith { .. } => Kind::InteractiveVirtualGridContent,
        Widget::VirtualGridWithSelection { .. } => Kind::VirtualGridWithSelection,
        Widget::VirtualGridContentWithSelection { .. } => Kind::VirtualGridContentWithSelection,
        Widget::VirtualGridWith { selection, .. } => match selection {
            super::super::VirtualSelectionWith::Single(_) => Kind::VirtualGrid,
            super::super::VirtualSelectionWith::Multiple { .. } => Kind::VirtualGridWithSelection,
        },
        Widget::VirtualGridContentWith { selection, .. } => match selection {
            super::super::VirtualSelectionWith::Single(_) => Kind::VirtualGridContent,
            super::super::VirtualSelectionWith::Multiple { .. } => {
                Kind::VirtualGridContentWithSelection
            }
        },
    }
}

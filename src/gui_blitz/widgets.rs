//! 新界面的通用控件与小部件。

use dioxus::prelude::*;

use super::State;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavigationIcon {
    Dashboard,
    Download,
    User,
    Settings,
    Tasks,
}

pub fn icon(kind: NavigationIcon, size: u32) -> Element {
    let size = size.to_string();
    match kind {
        NavigationIcon::Dashboard => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                rect { x: "3", y: "3", width: "7", height: "9", rx: "1" }
                rect { x: "14", y: "3", width: "7", height: "5", rx: "1" }
                rect { x: "14", y: "12", width: "7", height: "9", rx: "1" }
                rect { x: "3", y: "16", width: "7", height: "5", rx: "1" }
            }
        },
        NavigationIcon::Download => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M12 5v14" }
                path { d: "m19 12-7 7-7-7" }
            }
        },
        NavigationIcon::User => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }
                circle { cx: "12", cy: "7", r: "4" }
            }
        },
        NavigationIcon::Settings => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "3" }
                path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
            }
        },
        NavigationIcon::Tasks => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                path { d: "m3 17 2 2 4-4" }
                path { d: "m3 7 2 2 4-4" }
                path { d: "M13 6h8" }
                path { d: "M13 12h8" }
                path { d: "M13 18h8" }
            }
        },
    }
}

pub fn nav_item(
    mut state: Signal<State>,
    index: usize,
    kind: NavigationIcon,
    key: &'static str,
) -> Element {
    let active = state.read().tab == index;
    let label = state.read().t(key);
    let class = if active {
        "nav-item active"
    } else {
        "nav-item"
    };
    rsx! {
        div {
            class: "{class}",
            onclick: move |_| {
                let mut current = state.write();
                if current.tab != index {
                    current.prev_tab = current.tab;
                }
                current.tab = index;
                current.instance_page = 0;
                current.download_tab = 0;
            },
            {icon(kind, 16)}
            div { "{label}" }
        }
    }
}

pub fn nav_item_with_badge(
    mut state: Signal<State>,
    index: usize,
    kind: NavigationIcon,
    key: &'static str,
    count: usize,
) -> Element {
    let active = state.read().tab == index;
    let label = state.read().t(key);
    let class = if active {
        "nav-item active"
    } else {
        "nav-item"
    };
    rsx! {
        div {
            class: "{class}",
            onclick: move |_| {
                let mut current = state.write();
                if current.tab == index {
                    current.tab = current.prev_tab;
                } else {
                    current.prev_tab = current.tab;
                    current.tab = index;
                    current.instance_page = 0;
                }
            },
            {icon(kind, 16)}
            div { "{label}" }
            if count > 0 {
                div { class: "count", "{count}" }
            }
        }
    }
}

#[component]
pub fn text_input(
    value: String,
    on_change: EventHandler<String>,
    #[props(default = false)] inline: bool,
) -> Element {
    let class = if inline { "input inline" } else { "input" };
    rsx! {
        input {
            class: "{class}",
            value: "{value}",
            oninput: move |event| on_change.call(event.value()),
        }
    }
}

#[component]
pub fn toggle_switch(checked: bool, on_toggle: EventHandler<bool>) -> Element {
    let class = if checked { "switch on" } else { "switch" };
    rsx! {
        div {
            class: "{class}",
            onclick: move |_| on_toggle.call(!checked),
            div { class: "switch-thumb" }
        }
    }
}

pub fn separator() -> Element {
    rsx! { div { class: "sep" } }
}

pub fn list_logo<S: std::fmt::Display>(source: &Option<String>, fallback: S) -> Element {
    match source {
        Some(src) => rsx! { img { class: "logo", src: "{src}" } },
        None => rsx! { div { class: "logo-ph", "{fallback}" } },
    }
}

pub fn initial(title: &str) -> String {
    title
        .trim()
        .chars()
        .next()
        .map(|letter| letter.to_string())
        .unwrap_or_else(|| String::from("·"))
}

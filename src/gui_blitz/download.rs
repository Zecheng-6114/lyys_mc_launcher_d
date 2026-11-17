//! 下载页：实例安装、整合包导入、内容搜索与项目详情。

use dioxus::prelude::*;

use super::widgets::{initial, list_logo, separator, text_input, toggle_switch};
use super::{
    LoadState, PathTarget, State, add_mod, import_modrinth, import_pack, install_version,
    load_game_versions, load_loader_versions, load_mod_versions, open_mod_detail, pick_path,
    search_mods,
};
use crate::gui::clip_text;
use crate::modrinth;

fn row_title(text: String) -> Element {
    rsx! { div { class: "row-title", "{text}" } }
}

fn hint(text: String) -> Element {
    rsx! { div { class: "hint", "{text}" } }
}

pub fn download_page(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("page.download.title");
    let tabs = [
        (0, current.t("download.tab.instance")),
        (1, current.t("download.tab.pack")),
        (2, current.t("download.tab.mods")),
        (3, current.t("download.tab.resourcepack")),
        (4, current.t("download.tab.shader")),
        (5, current.t("download.tab.datapack")),
    ];
    let active_tab = current.download_tab;
    let detail = current.mod_detail.clone();
    drop(current);

    let body: Element = if active_tab == 0 {
        install_form(state)
    } else if let Some(hit) = detail {
        project_detail(state, hit, active_tab == 1)
    } else if active_tab == 1 {
        pack_search(state)
    } else {
        content_search(state)
    };

    rsx! {
        div { class: "page-body",
            div { class: "page-header",
                div { class: "page-title", "{title}" }
                div { class: "segmented",
                    for (index, label) in tabs {
                        {
                            let class = if active_tab == index { "seg active" } else { "seg" };
                            rsx! {
                                div {
                                    class: "{class}",
                                    key: "{index}",
                                    onclick: move |_| {
                                        let mut current = state.write();
                                        if current.download_tab != index {
                                            current.search_results.clear();
                                            current.search_results_state = LoadState::Idle;
                                            current.search_results_key = None;
                                            current.mod_detail = None;
                                            current.download_version_picker = false;
                                            current.download_instance_picker = false;
                                        }
                                        current.download_tab = index;
                                        if index == 2
                                            && !matches!(
                                                current.mod_loader.as_str(),
                                                "fabric" | "forge" | "neoforge"
                                            )
                                        {
                                            current.mod_loader = String::from("fabric");
                                        }
                                        if index == 4
                                            && current.mod_loader != "iris"
                                            && current.mod_loader != "optifine"
                                        {
                                            current.mod_loader = String::from("iris");
                                        }
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "scroll",
                div { class: "stack-lg", {body} }
            }
        }
    }
}

fn loader_segment(
    mut state: Signal<State>,
    values: &'static [&'static str],
    current_value: String,
) -> Element {
    rsx! {
        div { class: "segmented",
            for value in values {
                {
                    let active = current_value == *value;
                    let class = if active { "seg active" } else { "seg" };
                    let picked = value.to_string();
                    rsx! {
                        div {
                            class: "{class}",
                            key: "{value}",
                            onclick: move |_| {
                                let mut current = state.write();
                                current.mod_loader = if active { String::new() } else { picked.clone() };
                            },
                            "{value}"
                        }
                    }
                }
            }
        }
    }
}

fn install_form(mut state: Signal<State>) -> Element {
    let current = state.read();
    let version_title = current.t("install.version.title");
    let name_title = current.t("install.name.title");
    let loader_title = current.t("install.loader.title");
    let loader_version_title = current.t("install.loader_version.title");
    let memory_title = current.t("settings.memory.title");
    let java_title = current.t("settings.java.title");
    let mirror_title = current.t("settings.mirror.title");
    let maven_title = current.t("settings.maven.title");
    let isolate_title = current.t("install.isolate.title");
    let run_label = current.t("install.run");
    let browse = current.t("common.browse");
    let list_label = if current.install_version_picker {
        current.t("common.close")
    } else {
        current.t("common.list")
    };
    let loader_list_label = if current.install_loader_picker {
        current.t("common.close")
    } else {
        current.t("common.list")
    };
    let version = current.install_version.clone();
    let name = current.install_name.clone();
    let loader = current.install_loader.clone();
    let loader_version = current.install_loader_version.clone();
    let memory = current.install_memory.clone();
    let java = current.install_java.clone();
    let mirror = current.install_mirror.clone();
    let maven = current.install_maven.clone();
    let isolate = current.install_isolate;
    let version_picker = current.install_version_picker;
    let loader_picker = current.install_loader_picker;
    let picker_body = if version_picker {
        Some(version_picker_body(state, true))
    } else {
        None
    };
    let loader_body = if loader_picker {
        Some(loader_picker_body(state))
    } else {
        None
    };
    drop(current);

    rsx! {
        div { class: "panel",
            div { class: "row",
                {row_title(version_title)}
                div { class: "group",
                    text_input {
                        value: "{version}",
                        on_change: move |value| state.write().install_version = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.install_version_picker = !current.install_version_picker;
                            current.install_version_query.clear();
                        },
                        "{list_label}"
                    }
                }
            }
            if let Some(body) = picker_body.clone() {
                {separator()}
                {body}
            }
            {separator()}
            div { class: "row",
                {row_title(name_title)}
                text_input {
                    value: "{name}",
                    on_change: move |value| state.write().install_name = value,
                }
            }
            {separator()}
            div { class: "row",
                {row_title(loader_title)}
                div { class: "segmented",
                    for value in ["fabric", "forge", "neoforge"] {
                        {
                            let active = loader == value;
                            let class = if active { "seg active" } else { "seg" };
                            let picked = value.to_string();
                            rsx! {
                                div {
                                    class: "{class}",
                                    key: "{value}",
                                    onclick: move |_| {
                                        let mut current = state.write();
                                        current.install_loader = if active { String::new() } else { picked.clone() };
                                        current.install_loader_versions.clear();
                                        current.install_loader_versions_key = None;
                                        current.install_loader_versions_state = LoadState::Idle;
                                    },
                                    "{value}"
                                }
                            }
                        }
                    }
                }
            }
            if let Some(body) = loader_body.clone() {
                {separator()}
                {body}
            }
            {separator()}
            div { class: "row",
                {row_title(loader_version_title)}
                div { class: "group",
                    text_input {
                        value: "{loader_version}",
                        on_change: move |value| state.write().install_loader_version = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.install_loader_picker = !current.install_loader_picker;
                            current.install_loader_query.clear();
                            drop(current);
                            load_loader_versions(state);
                        },
                        "{loader_list_label}"
                    }
                }
            }
        }
        div { class: "panel",
            div { class: "row",
                {row_title(memory_title)}
                text_input {
                    value: "{memory}",
                    on_change: move |value| state.write().install_memory = value,
                }
            }
            {separator()}
            div { class: "row",
                {row_title(java_title)}
                div { class: "group",
                    text_input {
                        value: "{java}",
                        on_change: move |value| state.write().install_java = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| pick_path(state, PathTarget::InstallJava, String::new()),
                        "{browse}"
                    }
                }
            }
            {separator()}
            div { class: "row",
                {row_title(mirror_title)}
                text_input {
                    value: "{mirror}",
                    on_change: move |value| state.write().install_mirror = value,
                }
            }
            {separator()}
            div { class: "row",
                {row_title(maven_title)}
                text_input {
                    value: "{maven}",
                    on_change: move |value| state.write().install_maven = value,
                }
            }
        }
        div { class: "panel",
            div { class: "row",
                {row_title(isolate_title)}
                toggle_switch {
                    checked: isolate,
                    on_toggle: move |checked| state.write().install_isolate = checked,
                }
            }
        }
        div { class: "group", style: "justify-content: flex-end;",
            div {
                class: "btn btn-primary",
                onclick: move |_| install_version(state),
                "{run_label}"
            }
        }
    }
}

fn version_picker_body(mut state: Signal<State>, install: bool) -> Element {
    let current = state.read();
    let search_label = current.t("install.version.search");
    let close_label = current.t("common.close");
    let retry_label = current.t("common.retry");
    let loading = current.t("install.version.loading");
    let empty = current.t("install.version.empty");
    let query = if install {
        current.install_version_query.clone()
    } else {
        current.download_version_query.clone()
    };
    let state_value = current.install_versions_state.clone();
    let versions: Vec<(String, String)> = current
        .filter_versions()
        .iter()
        .map(|version| (version.id.clone(), current.version_kind(&version.kind)))
        .collect();
    drop(current);

    rsx! {
        div { class: "stack", style: "padding: 0 20px 14px 20px;",
            div { class: "stack",
                {hint(search_label)}
                div { class: "group",
                    text_input {
                        value: "{query}",
                        on_change: move |value| {
                            let mut current = state.write();
                            if install {
                                current.install_version_query = value;
                            } else {
                                current.download_version_query = value;
                            }
                        },
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            if install {
                                current.install_version_picker = false;
                                current.install_version_query.clear();
                            } else {
                                current.download_version_picker = false;
                                current.download_version_query.clear();
                            }
                        },
                        "{close_label}"
                    }
                }
            }
            match state_value {
                LoadState::Idle | LoadState::Loading => rsx! { {hint(loading)} },
                LoadState::Failed(error) => rsx! {
                    div { class: "stack",
                        div { class: "hint-warn", "{error}" }
                        div {
                            class: "btn-text",
                            onclick: move |_| load_game_versions(state),
                            "{retry_label}"
                        }
                    }
                },
                LoadState::Ready if versions.is_empty() => rsx! { {hint(empty)} },
                LoadState::Ready => rsx! {
                    div { class: "list scroll",
                        for (index, (id, kind)) in versions.into_iter().enumerate() {
                            div {
                                class: "item click",
                                key: "{index}",
                                onclick: {
                                    let id = id.clone();
                                    move |_| {
                                        let mut current = state.write();
                                        if install {
                                            current.install_version = id.clone();
                                            current.install_version_picker = false;
                                            current.install_version_query.clear();
                                            current.install_loader_versions.clear();
                                            current.install_loader_versions_key = None;
                                            current.install_loader_versions_state = LoadState::Idle;
                                        } else {
                                            current.mod_version = id.clone();
                                            current.download_version_picker = false;
                                            current.download_version_query.clear();
                                        }
                                    }
                                },
                                div { class: "item-text",
                                    div { class: "item-title", "{id}" }
                                }
                                div { class: "item-meta", "{kind}" }
                            }
                        }
                    }
                },
            }
        }
    }
}

fn loader_picker_body(mut state: Signal<State>) -> Element {
    let current = state.read();
    let search_label = current.t("install.loader.search");
    let close_label = current.t("common.close");
    let loading = current.t("install.loader.loading");
    let empty = current.t("install.loader.empty");
    let latest = current.t("install.loader.latest");
    let changed = current.t("install.loader.changed");
    let refresh = current.t("common.refresh");
    let retry = current.t("common.retry");
    let query = current.install_loader_query.clone();
    let loader = current.install_loader.clone();
    let version = current.install_version.clone();
    let current_key = (loader, version);
    let stale = current.install_loader_versions_key.as_ref() != Some(&current_key);
    let state_value = current.install_loader_versions_state.clone();
    let versions = current.filter_loader_versions();
    drop(current);

    rsx! {
        div { class: "stack", style: "padding: 0 20px 14px 20px;",
            div { class: "stack",
                {hint(search_label)}
                div { class: "group",
                    text_input {
                        value: "{query}",
                        on_change: move |value| state.write().install_loader_query = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.install_loader_picker = false;
                            current.install_loader_query.clear();
                        },
                        "{close_label}"
                    }
                }
            }
            match state_value {
                _ if stale => rsx! {
                    div { class: "stack",
                        {hint(changed)}
                        div {
                            class: "btn-text",
                            onclick: move |_| load_loader_versions(state),
                            "{refresh}"
                        }
                    }
                },
                LoadState::Idle | LoadState::Loading => rsx! { {hint(loading)} },
                LoadState::Failed(error) => rsx! {
                    div { class: "stack",
                        div { class: "hint-warn", "{error}" }
                        div {
                            class: "btn-text",
                            onclick: move |_| load_loader_versions(state),
                            "{retry}"
                        }
                    }
                },
                LoadState::Ready if versions.is_empty() => rsx! { {hint(empty)} },
                LoadState::Ready => rsx! {
                    div { class: "list scroll",
                        div {
                            class: "item click",
                            onclick: {
                                let latest = latest.clone();
                                move |_| {
                                    let mut current = state.write();
                                    current.install_loader_version.clear();
                                    current.install_loader_picker = false;
                                    current.install_loader_query.clear();
                                    let _ = &latest;
                                }
                            },
                            div { class: "item-text", div { class: "item-sub", "{latest}" } }
                        }
                        for (index, version) in versions.into_iter().enumerate() {
                            div {
                                class: "item click",
                                key: "{index}",
                                onclick: {
                                    let version = version.clone();
                                    move |_| {
                                        let mut current = state.write();
                                        current.install_loader_version = version.clone();
                                        current.install_loader_picker = false;
                                        current.install_loader_query.clear();
                                    }
                                },
                                div { class: "item-text", div { class: "item-title", "{version}" } }
                            }
                        }
                    }
                },
            }
        }
    }
}

fn instance_options(mut state: Signal<State>) -> Element {
    let current = state.read();
    let empty = current.t("instances.empty");
    let selected = current.mod_instance.trim().to_string();
    let rows: Vec<(String, String)> = current
        .instances
        .iter()
        .map(|row| {
            let meta = if row.loader.is_empty() {
                row.version.to_string()
            } else {
                format!("{} · {}", row.version, row.loader)
            };
            (row.name.to_string(), meta)
        })
        .collect();
    drop(current);

    rsx! {
        if rows.is_empty() {
            div { class: "empty-row", "{empty}" }
        } else {
            div { class: "list scroll",
                for (index, (name, meta)) in rows.into_iter().enumerate() {
                    {
                        let active = name == selected;
                        let class = if active { "item click active" } else { "item click" };
                        let picked = name.clone();
                        rsx! {
                            div {
                                class: "{class}",
                                key: "{index}",
                                onclick: move |_| {
                                    let mut current = state.write();
                                    current.mod_instance = picked.clone();
                                    current.download_instance_picker = false;
                                },
                                div { class: "item-text", div { class: "item-title", "{name}" } }
                                div { class: "item-meta", "{meta}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn content_search(mut state: Signal<State>) -> Element {
    let current = state.read();
    let query_title = current.t("download.pack.query.title");
    let version_title = current.t("download.version.title");
    let limit_title = current.t("download.limit.title");
    let instance_title = current.t("download.content.instance.title");
    let loader_title = if current.download_tab == 4 {
        current.t("download.content.shader_loader.title")
    } else {
        current.t("download.content.loader.title")
    };
    let pick_label = current.t("download.instance.pick");
    let search_label = current.t("download.content.search");
    let no_version = current.t("download.version.none");
    let query = current.mod_query.clone();
    let loader = current.mod_loader.clone();
    let instance = current.mod_instance.clone();
    let limit = current.mod_limit.clone();
    let tab = current.download_tab;
    let show_loader = tab == 2 || tab == 4;
    let picker_open = current.download_instance_picker;
    let detected = current
        .instances
        .iter()
        .find(|row| row.name.as_ref() == instance.as_str())
        .map(|row| row.version.to_string())
        .unwrap_or_else(|| no_version.clone());
    drop(current);

    rsx! {
        div { class: "panel",
            div { class: "row",
                {row_title(query_title)}
                text_input {
                    value: "{query}",
                    on_change: move |value| state.write().mod_query = value,
                }
            }
            if show_loader {
                {separator()}
                div { class: "row",
                    {row_title(loader_title)}
                    if tab == 2 {
                        {loader_segment(state, &["fabric", "forge", "neoforge"], loader.clone())}
                    } else {
                        {loader_segment(state, &["iris", "optifine"], loader.clone())}
                    }
                }
            }
            {separator()}
            div { class: "row",
                {row_title(instance_title)}
                div { class: "group",
                    text_input {
                        value: "{instance}",
                        on_change: move |value| state.write().mod_instance = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.download_instance_picker = !current.download_instance_picker;
                            let dir = current.dir.trim().to_string();
                            current.instances = crate::gui::list_instances(&dir);
                        },
                        "{pick_label}"
                    }
                }
            }
            if picker_open {
                {separator()}
                {instance_options(state)}
            }
            {separator()}
            div { class: "row",
                {row_title(version_title)}
                {hint(detected.clone())}
            }
            {separator()}
            div { class: "row",
                {row_title(limit_title)}
                text_input {
                    value: "{limit}",
                    on_change: move |value| state.write().mod_limit = value,
                }
            }
        }
        div { class: "group", style: "justify-content: flex-end;",
            div {
                class: "btn btn-primary",
                onclick: move |_| search_mods(state),
                "{search_label}"
            }
        }
        {search_results(state)}
    }
}

fn pack_search(mut state: Signal<State>) -> Element {
    let current = state.read();
    let file_title = current.t("download.import.file.title");
    let name_title = current.t("download.import.name.title");
    let optional_title = current.t("download.import.optional.title");
    let run_label = current.t("download.import.run");
    let query_title = current.t("download.pack.query.title");
    let version_title = current.t("download.version.title");
    let limit_title = current.t("download.limit.title");
    let search_label = current.t("download.pack.search");
    let browse = current.t("common.browse");
    let version_list_label = if current.download_version_picker {
        current.t("common.close")
    } else {
        current.t("common.list")
    };
    let file = current.import_file.clone();
    let name = current.import_name.clone();
    let optional = current.import_optional;
    let query = current.mod_query.clone();
    let version = current.mod_version.clone();
    let limit = current.mod_limit.clone();
    let picker_open = current.download_version_picker;
    let picker_body = if picker_open {
        Some(version_picker_body(state, false))
    } else {
        None
    };
    drop(current);

    rsx! {
        div { class: "panel",
            div { class: "row",
                {row_title(file_title)}
                div { class: "group",
                    text_input {
                        value: "{file}",
                        on_change: move |value| state.write().import_file = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| pick_path(state, PathTarget::ImportFile, String::new()),
                        "{browse}"
                    }
                }
            }
            {separator()}
            div { class: "row",
                {row_title(name_title)}
                text_input {
                    value: "{name}",
                    on_change: move |value| state.write().import_name = value,
                }
            }
            {separator()}
            div { class: "row",
                {row_title(optional_title)}
                toggle_switch {
                    checked: optional,
                    on_toggle: move |checked| state.write().import_optional = checked,
                }
            }
        }
        div { class: "group", style: "justify-content: flex-end;",
            div {
                class: "btn btn-primary",
                onclick: move |_| import_pack(state),
                "{run_label}"
            }
        }
        div { class: "panel",
            div { class: "row",
                {row_title(query_title)}
                text_input {
                    value: "{query}",
                    on_change: move |value| state.write().mod_query = value,
                }
            }
            {separator()}
            div { class: "row",
                {row_title(version_title)}
                div { class: "group",
                    text_input {
                        value: "{version}",
                        on_change: move |value| state.write().mod_version = value,
                    }
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.download_version_picker = !current.download_version_picker;
                            current.download_version_query.clear();
                        },
                        "{version_list_label}"
                    }
                }
            }
            if let Some(body) = picker_body.clone() {
                {separator()}
                {body}
            }
            {separator()}
            div { class: "row",
                {row_title(limit_title)}
                text_input {
                    value: "{limit}",
                    on_change: move |value| state.write().mod_limit = value,
                }
            }
        }
        div { class: "group", style: "justify-content: flex-end;",
            div {
                class: "btn btn-primary",
                onclick: move |_| search_mods(state),
                "{search_label}"
            }
        }
        {search_results(state)}
    }
}

fn search_results(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("download.results.title");
    let loading = current.t("download.results.loading");
    let empty = current.t("download.results.empty");
    let retry = current.t("common.retry");
    let state_value = current.search_results_state.clone();
    let rows: Vec<(String, String, String, String, String)> = current
        .search_results
        .iter()
        .map(|hit| {
            let icon = current.search_icons.get(&hit.slug).cloned();
            (
                hit.slug.clone(),
                hit.title.clone(),
                clip_text(&hit.description, 96),
                current.t_args(
                    "download.results.downloads",
                    &[("count", &hit.downloads.to_string())],
                ),
                icon.unwrap_or_default(),
            )
        })
        .collect();
    let hits: Vec<modrinth::Hit> = current.search_results.clone();
    drop(current);

    if matches!(state_value, LoadState::Idle) {
        return rsx! { div {} };
    }

    rsx! {
        div { class: "panel",
            div { class: "row", {row_title(title)} }
            {separator()}
            match state_value {
                LoadState::Idle => rsx! { div {} },
                LoadState::Loading => rsx! { div { class: "empty-row", "{loading}" } },
                LoadState::Failed(error) => rsx! {
                    div { class: "stack", style: "padding: 14px 20px;",
                        div { class: "hint-warn", "{error}" }
                        div {
                            class: "btn-text",
                            onclick: move |_| search_mods(state),
                            "{retry}"
                        }
                    }
                },
                LoadState::Ready if rows.is_empty() => rsx! { div { class: "empty-row", "{empty}" } },
                LoadState::Ready => rsx! {
                    div { class: "list scroll",
                        for (index, (slug, hit_title, description, downloads, icon)) in rows.into_iter().enumerate() {
                            {
                                let source = (!icon.is_empty()).then_some(icon.clone());
                                let letter = initial(&hit_title);
                                let target = hits.get(index).cloned();
                                let picked = slug.clone();
                                rsx! {
                                    div {
                                        class: "item click",
                                        key: "{slug}",
                                        onclick: move |_| {
                                            if let Some(hit) = target.clone() {
                                                state.write().mod_slug = picked.clone();
                                                open_mod_detail(state, hit);
                                            }
                                        },
                                        {list_logo(&source, letter)}
                                        div { class: "item-text",
                                            div { class: "item-title", "{hit_title}" }
                                            div { class: "item-sub", "{description}" }
                                        }
                                        div { class: "item-meta", "{downloads}" }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

fn project_detail(mut state: Signal<State>, hit: modrinth::Hit, pack: bool) -> Element {
    let current = state.read();
    let downloads = current.t_args(
        "download.results.downloads",
        &[("count", &hit.downloads.to_string())],
    );
    let description = clip_text(&hit.description, 160);
    let versions_title = if pack {
        current.t("download.pack.versions.title")
    } else {
        current.t("download.mod.versions.title")
    };
    let versions_loading = current.t("download.mod.versions.loading");
    let versions_empty = current.t("download.mod.versions.empty");
    let retry = current.t("common.retry");
    let back = current.t("download.mod.back");
    let name_title = current.t("download.import.name.title");
    let optional_title = current.t("download.import.optional.title");
    let instance_title = current.t("download.content.instance.title");
    let pick_label = current.t("download.instance.pick");
    let action_label = if pack {
        current.t("download.import.run")
    } else {
        current.t("download.content.add")
    };
    let versions_state = current.mod_versions_state.clone();
    let selected = current.effective_mod_version();
    let filtered = current.filtered_mod_versions();
    let versions: Vec<(String, String, String, String)> = filtered
        .iter()
        .map(|version| {
            let name = if version.name.is_empty() || version.name == version.number {
                String::new()
            } else {
                version.name.clone()
            };
            let loaders = version.loaders.join(", ");
            let date = version.date.get(..10).unwrap_or_default().to_string();
            let meta = if loaders.is_empty() {
                date
            } else {
                format!("{loaders} · {date}")
            };
            (version.id.clone(), version.number.clone(), name, meta)
        })
        .collect();
    let import_name = current.import_name.clone();
    let optional = current.import_optional;
    let instance = current.mod_instance.clone();
    let picker_open = current.download_instance_picker;
    let icon = current.search_icons.get(&hit.slug).cloned();
    let icon_source = icon.filter(|value| !value.is_empty());
    let icon_letter = initial(&hit.title);
    let title = hit.title.clone();
    let slug = hit.slug.clone();
    drop(current);

    rsx! {
        div { class: "panel",
            div { class: "row",
                div { class: "group",
                    {list_logo(&icon_source, icon_letter.clone())}
                    div { class: "row-label",
                        div { class: "row-title", "{title}" }
                        {hint(downloads.clone())}
                    }
                }
                div {
                    class: "btn-text",
                    onclick: move |_| state.write().mod_detail = None,
                    "{back}"
                }
            }
            {separator()}
            div { class: "row-desc", style: "padding: 10px 20px;", "{description}" }
        }
        div { class: "panel",
            div { class: "row", {row_title(versions_title)} }
            {separator()}
            match versions_state {
                LoadState::Idle | LoadState::Loading => rsx! { div { class: "empty-row", "{versions_loading}" } },
                LoadState::Failed(error) => rsx! {
                    div { class: "stack", style: "padding: 14px 20px;",
                        div { class: "hint-warn", "{error}" }
                        div {
                            class: "btn-text",
                            onclick: {
                                let slug = slug.clone();
                                move |_| load_mod_versions(state, slug.clone())
                            },
                            "{retry}"
                        }
                    }
                },
                LoadState::Ready if versions.is_empty() => rsx! { div { class: "empty-row", "{versions_empty}" } },
                LoadState::Ready => rsx! {
                    div { class: "list scroll", style: "max-height: 260px;",
                        for (index, (id, number, name, meta)) in versions.into_iter().enumerate() {
                            {
                                let active = selected.as_deref() == Some(id.as_str());
                                let class = if active { "item click active" } else { "item click" };
                                let picked = id.clone();
                                rsx! {
                                    div {
                                        class: "{class}",
                                        key: "{index}",
                                        onclick: move |_| {
                                            state.write().mod_version_selected = Some(picked.clone());
                                        },
                                        div { class: "item-text",
                                            div { class: "item-title", "{number}" }
                                            if !name.is_empty() {
                                                div { class: "item-sub", "{name}" }
                                            }
                                        }
                                        div { class: "item-meta", "{meta}" }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
        if pack {
            div { class: "panel",
                div { class: "row",
                    {row_title(name_title)}
                    text_input {
                        value: "{import_name}",
                        on_change: move |value| state.write().import_name = value,
                    }
                }
                {separator()}
                div { class: "row",
                    {row_title(optional_title)}
                    toggle_switch {
                        checked: optional,
                        on_toggle: move |checked| state.write().import_optional = checked,
                    }
                }
            }
        } else {
            div { class: "panel",
                div { class: "row",
                    {row_title(instance_title)}
                    div { class: "group",
                        text_input {
                            value: "{instance}",
                            on_change: move |value| state.write().mod_instance = value,
                        }
                        div {
                            class: "btn-text",
                            onclick: move |_| {
                                let mut current = state.write();
                                current.download_instance_picker = !current.download_instance_picker;
                                let dir = current.dir.trim().to_string();
                                current.instances = crate::gui::list_instances(&dir);
                            },
                            "{pick_label}"
                        }
                    }
                }
                if picker_open {
                    {separator()}
                    {instance_options(state)}
                }
            }
        }
        div { class: "group", style: "justify-content: flex-end;",
            if selected.is_some() {
                div {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        if pack {
                            import_modrinth(state);
                        } else {
                            add_mod(state);
                        }
                    },
                    "{action_label}"
                }
            } else {
                div { class: "btn btn-disabled", "{action_label}" }
            }
        }
    }
}

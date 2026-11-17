//! 实例、设置、账号与任务页面的渲染。

use dioxus::prelude::*;

use super::widgets::{separator, text_input, toggle_switch};
use super::{
    LoadState, PathTarget, State, TaskState, auto_select_instance_java, back_to_instances,
    cancel_task, detect_java, launch_instance, load_mods, login_microsoft, open_folder,
    open_instance_settings, pick_java_runtime, pick_path, remove_account, remove_mod,
    save_instance_settings, switch_account, toggle_mod,
};

fn row_title(text: String) -> Element {
    rsx! { div { class: "row-title", "{text}" } }
}

fn row_desc(text: String) -> Element {
    rsx! { div { class: "row-desc", "{text}" } }
}

fn hint(text: String) -> Element {
    rsx! { div { class: "hint", "{text}" } }
}

pub fn page(state: Signal<State>) -> Element {
    let (instance_page, tab) = {
        let current = state.read();
        (current.instance_page, current.tab)
    };
    if instance_page == 2 {
        return instance_settings_page(state);
    }
    match tab {
        0 => instances(state),
        1 => super::download::download_page(state),
        3 => tasks(state),
        4 => accounts(state),
        _ => settings(state),
    }
}

// ---------- 实例 ----------

fn instances(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("page.instances.title");
    let refresh = current.t("common.refresh");
    let empty = current.t("instances.empty");
    let launch_label = current.t("instances.action.launch");
    let cancel_label = current.t("instances.action.cancel");
    let folder_label = current.t("instances.action.folder");
    let settings_label = current.t("instances.action.settings");
    let isolated_label = current.t("instances.meta.isolated");
    let rows: Vec<(String, String, Option<usize>)> = current
        .instances
        .iter()
        .map(|row| {
            let mut meta = format!("{} · {}", row.version, row.loader);
            if row.isolated {
                meta.push_str(" · ");
                meta.push_str(&isolated_label);
            }
            (
                row.name.to_string(),
                meta,
                current.running_launch_for(row.name.as_ref()),
            )
        })
        .collect();
    drop(current);

    rsx! {
        div { class: "page-body",
            div { class: "page-header",
                div { class: "page-title", "{title}" }
                div { class: "group",
                    div {
                        class: "btn-text",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.reload();
                        },
                        "{refresh}"
                    }
                }
            }
            div { class: "scroll",
                div { class: "panel",
                    if rows.is_empty() {
                        div { class: "empty", "{empty}" }
                    }
                    for (index, (name, meta, running)) in rows.into_iter().enumerate() {
                        if index > 0 {
                            {separator()}
                        }
                        div { class: "row",
                            div { class: "row-label",
                                div { class: "row-title", "{name}" }
                                div { class: "row-desc", "{meta}" }
                            }
                            div { class: "group",
                                if let Some(task_id) = running {
                                    div {
                                        class: "btn btn-danger",
                                        onclick: move |_| cancel_task(state, task_id),
                                        "{cancel_label}"
                                    }
                                } else {
                                    {
                                        let target = name.clone();
                                        rsx! {
                                            div {
                                                class: "btn btn-primary",
                                                onclick: move |_| launch_instance(state, &target),
                                                "{launch_label}"
                                            }
                                        }
                                    }
                                }
                                {
                                    let target = name.clone();
                                    rsx! {
                                        div {
                                            class: "btn-text",
                                            onclick: move |_| open_folder(state, target.clone()),
                                            "{folder_label}"
                                        }
                                    }
                                }
                                {
                                    let target = name.clone();
                                    rsx! {
                                        div {
                                            class: "btn-text",
                                            onclick: move |_| open_instance_settings(state, target.clone()),
                                            "{settings_label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------- 通用设置 ----------

fn settings(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("page.settings.title");
    let game_dir_title = current.t("settings.game_dir.title");
    let memory_title = current.t("settings.memory.title");
    let java_title = current.t("settings.java.title");
    let java_detect = current.t("settings.java.detect");
    let java_scan = current.t("settings.java.scan_dir");
    let mirror_title = current.t("settings.mirror.title");
    let maven_title = current.t("settings.maven.title");
    let language_title = current.t("settings.language.title");
    let ui_title = current.t("settings.ui.title");
    let ui_hint = current.t("settings.ui.hint");
    let isolate_title = current.t("settings.isolate.title");
    let save_label = current.t("settings.save");
    let browse = current.t("common.browse");
    let java_detecting = current.t("settings.java.detecting");
    let java_none = current.t("settings.java.none");
    let dir = current.dir.clone();
    let memory = current.memory.clone();
    let java = current.java.clone();
    let mirror = current.mirror.clone();
    let maven = current.maven.clone();
    let isolate = current.isolate;
    let experimental = current.experimental_ui;
    let saved = current.settings_value() == current.saved_settings;
    let lang = current.lang;
    let java_picker = current.java_picker;
    let java_state = current.java_runtimes_state.clone();
    let java_runtimes: Vec<(String, String, String)> = current
        .java_runtimes
        .iter()
        .map(|runtime| {
            (
                format!("Java {}", runtime.major),
                runtime.arch.clone(),
                runtime.path.clone(),
            )
        })
        .collect();
    drop(current);

    let languages = [
        ("zh-CN", "简体中文"),
        ("en-US", "English"),
        ("ja-JP", "日本語"),
    ];

    rsx! {
        div { class: "page-body",
            div { class: "page-title", "{title}" }
            div { class: "scroll",
                div { class: "stack",
                    div { class: "panel",
                        div { class: "row",
                            {row_title(game_dir_title.clone())}
                            div { class: "group",
                                text_input {
                                    value: "{dir}",
                                    on_change: move |value| state.write().dir = value,
                                }
                                div {
                                    class: "btn-text",
                                    onclick: move |_| pick_path(state, PathTarget::GameDir, game_dir_title.clone()),
                                    "{browse}"
                                }
                            }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(memory_title)}
                            text_input {
                                value: "{memory}",
                                on_change: move |value| state.write().memory = value,
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            {row_title(java_title.clone())}
                            div { class: "group",
                                text_input {
                                    value: "{java}",
                                    on_change: move |value| state.write().java = value,
                                }
                                {
                                    let java_title = java_title.clone();
                                    rsx! {
                                        div {
                                            class: "btn-text",
                                            onclick: move |_| pick_path(state, PathTarget::Java, java_title.clone()),
                                            "{browse}"
                                        }
                                    }
                                }
                                div {
                                    class: "btn-text",
                                    onclick: move |_| detect_java(state),
                                    "{java_detect}"
                                }
                                {
                                    let java_scan_title = java_title.clone();
                                    rsx! {
                                        div {
                                            class: "btn-text",
                                            onclick: move |_| pick_path(state, PathTarget::ScanJavaDir, java_scan_title.clone()),
                                            "{java_scan}"
                                        }
                                    }
                                }
                            }
                        }
                        if java_picker {
                            {separator()}
                            div { class: "stack", style: "padding: 0 20px 12px 20px;",
                                match java_state {
                                    LoadState::Idle | LoadState::Loading => rsx! {
                                        {hint(java_detecting)}
                                    },
                                    LoadState::Failed(error) => rsx! {
                                        div { class: "hint-warn", "{error}" }
                                    },
                                    LoadState::Ready if java_runtimes.is_empty() => rsx! {
                                        {hint(java_none)}
                                    },
                                    LoadState::Ready => rsx! {
                                        div { class: "list",
                                            for (index, (label, arch, path)) in java_runtimes.iter().enumerate() {
                                                div {
                                                    class: "item click",
                                                    key: "{index}",
                                                    onclick: move |_| pick_java_runtime(state, index),
                                                    div { class: "group",
                                                        div { style: "font-size: 12.5px;", "{label}" }
                                                        div { class: "hint", "{arch}" }
                                                    }
                                                    div { class: "item-sub truncate", "{path}" }
                                                }
                                            }
                                        }
                                    },
                                }
                            }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(mirror_title)}
                            text_input {
                                value: "{mirror}",
                                on_change: move |value| state.write().mirror = value,
                            }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(maven_title)}
                            text_input {
                                value: "{maven}",
                                on_change: move |value| state.write().maven = value,
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            {row_title(language_title)}
                            div { class: "segmented",
                                for (code, label) in languages {
                                    {
                                        let active = lang.tag() == code;
                                        let class = if active { "seg active" } else { "seg" };
                                        rsx! {
                                            div {
                                                class: "{class}",
                                                onclick: move |_| {
                                                    if let Some(target) = crate::i18n::Lang::from_tag(code) {
                                                        state.write().select_language(target);
                                                    }
                                                },
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            div { class: "row-label",
                                {row_title(ui_title)}
                                {row_desc(ui_hint)}
                            }
                            toggle_switch {
                                checked: experimental,
                                on_toggle: move |checked| {
                                    let mut current = state.write();
                                    current.experimental_ui = checked;
                                    current.save_settings();
                                },
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            {row_title(isolate_title)}
                            toggle_switch {
                                checked: isolate,
                                on_toggle: move |checked| state.write().isolate = checked,
                            }
                        }
                    }
                    div { class: "group", style: "justify-content: flex-end; width: 100%;",
                        if saved {
                            div { class: "btn btn-disabled", "{save_label}" }
                        } else {
                            div {
                                class: "btn btn-primary",
                                onclick: move |_| state.write().save_settings(),
                                "{save_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------- 账号 ----------

fn accounts(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("page.accounts.title");
    let add = current.t("accounts.add");
    let microsoft = current.t("accounts.add.microsoft");
    let offline = current.t("accounts.add.offline");
    let empty = current.t("accounts.empty");
    let name_title = current.t("accounts.offline.name");
    let cancel = current.t("common.cancel");
    let add_confirm = current.t("common.add");
    let switch_label = current.t("accounts.switch");
    let remove_label = current.t("accounts.remove");
    let current_label = current.t("accounts.current");
    let offline_name = current.offline_name.clone();
    let menu_open = current.account_menu_open;
    let form_open = current.offline_form_open;
    let active_uuid = crate::microsoft::current_account()
        .map(|account| account.uuid)
        .unwrap_or_default();
    let rows: Vec<(String, String, String, bool)> = current
        .accounts
        .iter()
        .map(|account| {
            let desc = if account.offline {
                current.t_args("accounts.uuid.offline", &[("uuid", &account.uuid)])
            } else {
                current.t_args("accounts.uuid", &[("uuid", &account.uuid)])
            };
            let is_current = account.uuid == active_uuid;
            (account.name.clone(), account.uuid.clone(), desc, is_current)
        })
        .collect();
    drop(current);

    rsx! {
        div { class: "page-body",
            div { class: "page-header-center",
                div { class: "page-title", "{title}" }
                div {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        let mut current = state.write();
                        current.account_menu_open = !current.account_menu_open;
                        current.offline_form_open = false;
                    },
                    "{add}"
                    {super::widgets::icon(super::widgets::NavigationIcon::Download, 13)}
                }
            }
            div { class: "scroll",
                div { class: "stack",
                    if menu_open {
                        div { class: "panel",
                            div {
                                class: "row click",
                                onclick: move |_| {
                                    state.write().account_menu_open = false;
                                    login_microsoft(state);
                                },
                                {row_title(microsoft)}
                            }
                            {separator()}
                            div {
                                class: "row click",
                                onclick: move |_| {
                                    let mut current = state.write();
                                    current.account_menu_open = false;
                                    current.offline_form_open = true;
                                },
                                {row_title(offline)}
                            }
                        }
                    }
                    if form_open {
                        div { class: "panel",
                            div { class: "row",
                                {row_title(name_title)}
                                text_input {
                                    value: "{offline_name}",
                                    on_change: move |value| state.write().offline_name = value,
                                }
                            }
                            div { class: "group", style: "justify-content: flex-end; padding: 0 20px 14px 20px;",
                                div {
                                    class: "btn-text",
                                    onclick: move |_| state.write().offline_form_open = false,
                                    "{cancel}"
                                }
                                div {
                                    class: "btn btn-primary",
                                    onclick: move |_| super::add_offline_account(state),
                                    "{add_confirm}"
                                }
                            }
                        }
                    }
                    div { class: "panel",
                        if rows.is_empty() {
                            div { class: "empty-row", "{empty}" }
                        }
                        for (index, (name, key, desc, is_current)) in rows.into_iter().enumerate() {
                            if index > 0 {
                                {separator()}
                            }
                            div { class: "row",
                                div { class: "row-label",
                                    div { class: "row-title", "{name}" }
                                    div { class: "row-desc", "{desc}" }
                                }
                                div { class: "group",
                                    if is_current {
                                        div { class: "badge", "{current_label}" }
                                    } else {
                                        {
                                            let account_name = name.clone();
                                            let account_key = key.clone();
                                            rsx! {
                                                div {
                                                    class: "btn-text",
                                                    onclick: move |_| switch_account(state, account_name.clone(), account_key.clone()),
                                                    "{switch_label}"
                                                }
                                            }
                                        }
                                    }
                                    {
                                        let account_name = name.clone();
                                        let account_key = key.clone();
                                        rsx! {
                                            div {
                                                class: "btn btn-danger",
                                                onclick: move |_| remove_account(state, account_name.clone(), account_key.clone()),
                                                "{remove_label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------- 任务 ----------

fn task_state_key(state: TaskState) -> &'static str {
    match state {
        TaskState::Running => "task.state.running",
        TaskState::Success => "task.state.success",
        TaskState::Failed => "task.state.failed",
        TaskState::Cancelled => "task.state.cancelled",
    }
}

fn tasks(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("page.tasks.title");
    let empty = current.t("tasks.empty");
    let detail_empty = current.t("tasks.detail.empty");
    let log_empty = current.t("tasks.log.empty");
    let cancel = current.t("tasks.cancel");
    let active = current.active_task;
    let rows: Vec<(usize, String, TaskState, Option<String>, String)> = current
        .tasks
        .iter()
        .rev()
        .map(|task| {
            (
                task.id,
                task.title.clone(),
                task.state,
                task.progress.map(|value| format!("{:.0}%", value * 100.0)),
                current.t(task_state_key(task.state)),
            )
        })
        .collect();
    let selected: Option<(
        usize,
        String,
        TaskState,
        Option<String>,
        String,
        Vec<String>,
    )> = current
        .active_task
        .and_then(|id| current.tasks.iter().find(|task| task.id == id))
        .map(|task| {
            let skip = task.log_lines.len().saturating_sub(400);
            (
                task.id,
                task.title.clone(),
                task.state,
                task.progress.map(|value| format!("{:.0}%", value * 100.0)),
                current.t_args(
                    "tasks.detail.header",
                    &[
                        ("id", &task.id.to_string()),
                        ("state", &current.t(task_state_key(task.state))),
                    ],
                ),
                task.log_lines
                    .iter()
                    .skip(skip)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        });
    drop(current);

    rsx! {
        div { class: "page-body",
            div { class: "page-title", "{title}" }
            div { class: "tasks",
                div { class: "task-list",
                    if rows.is_empty() {
                        div { class: "empty-row", "{empty}" }
                    }
                    for (id, task_title, task_state, percent, label) in rows.into_iter() {
                        {
                            let color = match task_state {
                                TaskState::Running => "var(--busy)",
                                TaskState::Success => "var(--ok)",
                                TaskState::Failed => "var(--danger-hover)",
                                TaskState::Cancelled => "var(--faint)",
                            };
                            let class = if active == Some(id) { "task-item active" } else { "task-item" };
                            rsx! {
                                div {
                                    class: "{class}",
                                    key: "{id}",
                                    onclick: move |_| {
                                        let mut current = state.write();
                                        current.active_task = Some(id);
                                        current.task_log_follow = true;
                                    },
                                    div { class: "dot", style: "background: {color};" }
                                    div { class: "item-text",
                                        div { class: "truncate", style: "font-size: 12.5px; color: var(--text);", "{task_title}" }
                                        div { class: "item-sub", "{label}" }
                                    }
                                    if let Some(percent) = percent {
                                        div { class: "item-sub", "{percent}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "task-detail",
                    if let Some((id, task_title, task_state, percent, header, lines)) = selected {
                        div { class: "page-header-center",
                            div { class: "row-label",
                                div { class: "truncate", style: "font-size: 16px; color: var(--text);", "{task_title}" }
                                div { class: "hint", "{header}" }
                            }
                            if task_state == TaskState::Running {
                                div {
                                    class: "btn btn-danger",
                                    onclick: move |_| cancel_task(state, id),
                                    "{cancel}"
                                }
                            }
                        }
                        if let Some(percent) = percent {
                            div { class: "group",
                                div { class: "hint", "{percent}" }
                            }
                        }
                        div { class: "task-log",
                            if lines.is_empty() {
                                div { class: "task-log-line", "{log_empty}" }
                            }
                            for (index, line) in lines.into_iter().enumerate() {
                                div { class: "task-log-line", key: "{index}", "{line}" }
                            }
                        }
                    } else {
                        div { class: "hint", "{detail_empty}" }
                    }
                }
            }
        }
    }
}

// ---------- 实例设置 ----------

fn instance_settings_page(mut state: Signal<State>) -> Element {
    let current = state.read();
    let title = current.t("instance_settings.title");
    let instance_title = current.t("instance_settings.instance");
    let override_hint = current.t("instance_settings.override");
    let memory_title = current.t("settings.memory.title");
    let java_title = current.t("settings.java.title");
    let java_auto = current.t("settings.java.auto");
    let mirror_title = current.t("settings.mirror.title");
    let maven_title = current.t("settings.maven.title");
    let isolate_title = current.t("instance_settings.isolate.title");
    let source_local = current.t("instance_settings.source.local");
    let source_unset = current.t("instance_settings.source.unset");
    let source_global = current.t("instance_settings.source.global");
    let reset_label = current.t("instance_settings.reset");
    let save_label = current.t("instance_settings.save");
    let mods_title = current.t("instance_settings.mods.title");
    let mods_empty = current.t("instance_settings.mods.empty");
    let mods_remove = current.t("instance_settings.mods.remove");
    let mods_unknown = current.t("instance_settings.mods.unknown");
    let refresh = current.t("common.refresh");
    let browse = current.t("common.browse");
    let name = current.cfg_name.clone();
    let cfg_memory = current.cfg_memory.clone();
    let cfg_java = current.cfg_java.clone();
    let cfg_mirror = current.cfg_mirror.clone();
    let cfg_maven = current.cfg_maven.clone();
    let cfg_isolate = current.cfg_isolate;
    let global_memory = current
        .memory_value(&current.memory)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let global_java = current.java.trim().to_string();
    let global_mirror = current.mirror.trim().to_string();
    let global_maven = current.maven.trim().to_string();
    let java_note = current.cfg_java_note.clone();
    let java_required = current.cfg_java_required.map(|required| {
        current.t_args(
            "settings.java.required",
            &[("major", &required.to_string())],
        )
    });
    let saved = current.cfg_current() == current.cfg_saved;
    let isolate_local = current.cfg_isolate_local;
    let memory_source = if cfg_memory.trim().is_empty() {
        format!("{source_global}: {global_memory}")
    } else {
        source_local.clone()
    };
    let java_source = if cfg_java.trim().is_empty() {
        format!("{source_global}: {global_java}")
    } else {
        source_local.clone()
    };
    let mirror_source = if cfg_mirror.trim().is_empty() {
        format!("{source_global}: {global_mirror}")
    } else {
        source_local.clone()
    };
    let maven_source = if cfg_maven.trim().is_empty() {
        format!("{source_global}: {global_maven}")
    } else {
        source_local.clone()
    };
    let mods: Vec<(String, String, String, bool)> = current
        .mods
        .iter()
        .map(|entry| {
            let label = if entry.version.is_empty() {
                entry.id.clone()
            } else {
                format!("{} {}", entry.id, entry.version)
            };
            let source = entry.source.clone().unwrap_or_else(|| mods_unknown.clone());
            (entry.file.clone(), label, source, entry.disabled)
        })
        .collect();
    drop(current);

    rsx! {
        div { class: "page-body",
            div { class: "page-header", style: "gap: 10px; justify-content: flex-start;",
                div {
                    class: "btn-text",
                    onclick: move |_| back_to_instances(state),
                    "←"
                }
                div { class: "page-title", "{title}" }
            }
            div { class: "scroll",
                div { class: "stack",
                    div { class: "panel",
                        div { class: "row",
                            div { class: "row-label",
                                {row_title(instance_title)}
                                {row_desc(name.clone())}
                            }
                            div { class: "row-desc", "{override_hint}" }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(memory_title)}
                            div { class: "group",
                                text_input {
                                    value: "{cfg_memory}",
                                    on_change: move |value| state.write().cfg_memory = value,
                                }
                                {hint(memory_source.clone())}
                                if !cfg_memory.trim().is_empty() {
                                    div {
                                        class: "btn-text",
                                        onclick: move |_| state.write().cfg_memory.clear(),
                                        "{reset_label}"
                                    }
                                }
                            }
                        }
                        {separator()}
                        div { class: "row",
                            div { class: "row-label",
                                {row_title(java_title)}
                                if let Some(note) = java_note.clone() {
                                    div { class: "hint-warn", "{note}" }
                                }
                                if let Some(required) = java_required.clone() {
                                    {hint(required)}
                                }
                            }
                            div { class: "group",
                                text_input {
                                    value: "{cfg_java}",
                                    on_change: move |value| state.write().cfg_java = value,
                                }
                                div {
                                    class: "btn-text",
                                    onclick: move |_| pick_path(state, PathTarget::InstanceJava, String::new()),
                                    "{browse}"
                                }
                                div {
                                    class: "btn-text",
                                    onclick: move |_| auto_select_instance_java(state),
                                    "{java_auto}"
                                }
                                {hint(java_source.clone())}
                                if !cfg_java.trim().is_empty() {
                                    div {
                                        class: "btn-text",
                                        onclick: move |_| state.write().cfg_java.clear(),
                                        "{reset_label}"
                                    }
                                }
                            }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(mirror_title)}
                            div { class: "group",
                                text_input {
                                    value: "{cfg_mirror}",
                                    on_change: move |value| state.write().cfg_mirror = value,
                                }
                                {hint(mirror_source.clone())}
                            }
                        }
                        {separator()}
                        div { class: "row",
                            {row_title(maven_title)}
                            div { class: "group",
                                text_input {
                                    value: "{cfg_maven}",
                                    on_change: move |value| state.write().cfg_maven = value,
                                }
                                {hint(maven_source.clone())}
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            {row_title(isolate_title)}
                            div { class: "group",
                                toggle_switch {
                                    checked: cfg_isolate,
                                    on_toggle: move |checked| {
                                        let mut current = state.write();
                                        current.cfg_isolate = checked;
                                        current.cfg_isolate_local = true;
                                    },
                                }
                                {hint(if isolate_local { source_local.clone() } else { source_unset.clone() })}
                            }
                        }
                    }
                    div { class: "panel",
                        div { class: "row",
                            {row_title(mods_title)}
                            {
                                let target = name.clone();
                                rsx! {
                                    div {
                                        class: "btn-text",
                                        onclick: move |_| load_mods(state, target.clone()),
                                        "{refresh}"
                                    }
                                }
                            }
                        }
                        {separator()}
                        div { class: "list",
                            if mods.is_empty() {
                                {hint(mods_empty)}
                            }
                            for (file, label, source, disabled) in mods.into_iter() {
                                div { class: "item", key: "{file}",
                                    div { class: "item-text",
                                        div { class: "truncate", "{label}" }
                                        div { class: "item-sub truncate", "{source}" }
                                    }
                                    toggle_switch {
                                        checked: !disabled,
                                        on_toggle: {
                                            let file = file.clone();
                                            move |_| toggle_mod(state, file.clone())
                                        },
                                    }
                                    div {
                                        class: "btn btn-danger",
                                        onclick: move |_| remove_mod(state, file.clone()),
                                        "{mods_remove}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "group", style: "justify-content: flex-end; width: 100%;",
                        if saved {
                            div { class: "btn btn-disabled", "{save_label}" }
                        } else {
                            div {
                                class: "btn btn-primary",
                                onclick: move |_| save_instance_settings(state),
                                "{save_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

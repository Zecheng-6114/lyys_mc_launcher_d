//! 实验性界面：Dioxus + Blitz（原生 HTML/CSS 渲染）。
//!
//! 与 [`crate::gui`] 并行存在，由配置项 `experimental_ui` 或启动参数
//! `--ui blitz` 选择。业务侧仍复用 `crate::gui` 中的任务执行、配置读写与
//! 实例扫描逻辑，本模块只负责界面表达与状态持有。

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use dioxus::prelude::*;

use crate::catalog::{self, GameVersion};
use crate::gui::{
    InstanceRow, TaskEvent, TaskHandle, execute, kill_tree, list_instances, load_settings,
    non_empty, open_directory, push_option, split_values, write_json,
};
use crate::i18n::{self, Lang};
use crate::instance::{self, LaunchConfig};
use crate::modrinth;
use crate::mods;

mod download;
mod views;
mod widgets;

pub(crate) const CSS: &str = include_str!("theme.css");
pub(crate) const CONFIG_TASK: &str = "microsoft";

pub fn run() -> Result<(), Box<dyn Error>> {
    let attributes = dioxus::native::WindowAttributes::default()
        .with_title("LYYS MC Launcher")
        .with_surface_size(dioxus::native::LogicalSize::new(1180.0, 800.0))
        .with_min_surface_size(dioxus::native::LogicalSize::new(980.0, 640.0));
    let config = dioxus::native::Config::new().with_window_attributes(attributes);
    dioxus::native::launch_cfg_with_props(app, (), vec![], vec![Box::new(config)]);
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum LoadState {
    Idle,
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskState {
    Running,
    Success,
    Failed,
    Cancelled,
}

pub(crate) struct TaskEntry {
    pub id: usize,
    pub title: String,
    pub instance: Option<String>,
    pub command: String,
    pub key: String,
    pub state: TaskState,
    pub progress: Option<f32>,
    pub log_lines: std::collections::VecDeque<String>,
    pub last_log_notify: Instant,
    pub handle: Arc<TaskHandle>,
}

pub(crate) struct State {
    pub tab: usize,
    pub prev_tab: usize,
    pub instance_page: usize,
    pub download_tab: usize,
    pub lang: Lang,
    pub language: Option<String>,
    pub saved_settings: serde_json::Value,
    pub experimental_ui: bool,

    pub instances: Vec<InstanceRow>,
    pub account: Option<String>,
    pub accounts: Vec<crate::microsoft::MinecraftAccount>,
    pub account_offline: bool,
    pub account_menu_open: bool,
    pub offline_form_open: bool,
    pub offline_name: String,

    pub tasks: Vec<TaskEntry>,
    pub active_task: Option<usize>,
    pub next_task_id: usize,
    pub task_log_follow: bool,

    pub dir: String,
    pub memory: String,
    pub java: String,
    pub mirror: String,
    pub maven: String,
    pub isolate: bool,

    pub install_version: String,
    pub install_name: String,
    pub install_loader: String,
    pub install_loader_version: String,
    pub install_memory: String,
    pub install_java: String,
    pub install_mirror: String,
    pub install_maven: String,
    pub install_isolate: bool,
    pub install_versions: Vec<GameVersion>,
    pub install_versions_state: LoadState,
    pub install_version_picker: bool,
    pub install_version_query: String,
    pub install_loader_versions: Vec<String>,
    pub install_loader_versions_state: LoadState,
    pub install_loader_versions_key: Option<(String, String)>,
    pub install_loader_picker: bool,
    pub install_loader_query: String,
    pub download_version_picker: bool,
    pub download_version_query: String,

    pub cfg_name: String,
    pub cfg_memory: String,
    pub cfg_java: String,
    pub cfg_mirror: String,
    pub cfg_maven: String,
    pub cfg_isolate: bool,
    pub cfg_isolate_local: bool,
    pub cfg_java_required: Option<u32>,
    pub cfg_java_note: Option<String>,
    pub cfg_saved: LaunchConfig,
    pub java_runtimes: Vec<crate::java::JavaRuntime>,
    pub java_runtimes_state: LoadState,
    pub java_picker: bool,
    pub mods: Vec<mods::ModEntry>,

    pub import_file: String,
    pub import_name: String,
    pub import_optional: bool,

    pub mod_query: String,
    pub mod_loader: String,
    pub mod_version: String,
    pub mod_limit: String,
    pub mod_slug: String,
    pub mod_instance: String,
    pub search_results: Vec<modrinth::Hit>,
    pub search_results_state: LoadState,
    pub search_results_key: Option<String>,
    pub search_icons: HashMap<String, String>,
    pub mod_detail: Option<modrinth::Hit>,
    pub mod_versions: Vec<modrinth::ProjectVersion>,
    pub mod_versions_state: LoadState,
    pub mod_versions_key: Option<String>,
    pub mod_version_selected: Option<String>,
    pub download_instance_picker: bool,
}

impl State {
    fn new() -> Self {
        let settings = load_settings();
        let memory = settings.memory.unwrap_or(4096).min(65536).to_string();
        let lang = settings
            .language
            .as_deref()
            .and_then(Lang::from_tag)
            .unwrap_or(i18n::FALLBACK);
        let mut state = State {
            tab: 0,
            prev_tab: 0,
            instance_page: 0,
            download_tab: 0,
            lang,
            language: settings.language.clone(),
            saved_settings: serde_json::Value::Null,
            experimental_ui: settings.experimental_ui,
            instances: Vec::new(),
            account: None,
            accounts: Vec::new(),
            account_offline: false,
            account_menu_open: false,
            offline_form_open: false,
            offline_name: String::new(),
            tasks: Vec::new(),
            active_task: None,
            next_task_id: 1,
            task_log_follow: false,
            dir: settings.game_dir.clone(),
            memory: memory.clone(),
            java: settings.java.clone(),
            mirror: settings.mirror.clone(),
            maven: settings.maven.clone(),
            isolate: settings.isolate,
            install_version: String::from("1.20.1"),
            install_name: String::new(),
            install_loader: String::new(),
            install_loader_version: String::new(),
            install_memory: memory,
            install_java: settings.java.clone(),
            install_mirror: settings.mirror.clone(),
            install_maven: settings.maven.clone(),
            install_isolate: settings.isolate,
            install_versions: Vec::new(),
            install_versions_state: LoadState::Idle,
            install_version_picker: false,
            install_version_query: String::new(),
            install_loader_versions: Vec::new(),
            install_loader_versions_state: LoadState::Idle,
            install_loader_versions_key: None,
            install_loader_picker: false,
            install_loader_query: String::new(),
            download_version_picker: false,
            download_version_query: String::new(),
            cfg_name: String::new(),
            cfg_memory: String::new(),
            cfg_java: settings.java.clone(),
            cfg_mirror: settings.mirror.clone(),
            cfg_maven: settings.maven.clone(),
            cfg_isolate: false,
            cfg_isolate_local: false,
            cfg_java_required: None,
            cfg_java_note: None,
            cfg_saved: LaunchConfig::default(),
            java_runtimes: Vec::new(),
            java_runtimes_state: LoadState::Idle,
            java_picker: false,
            mods: Vec::new(),
            import_file: String::new(),
            import_name: String::new(),
            import_optional: false,
            mod_query: String::new(),
            mod_loader: String::new(),
            mod_version: String::new(),
            mod_limit: String::from("10"),
            mod_slug: String::new(),
            mod_instance: String::new(),
            search_results: Vec::new(),
            search_results_state: LoadState::Idle,
            search_results_key: None,
            search_icons: HashMap::new(),
            mod_detail: None,
            mod_versions: Vec::new(),
            mod_versions_state: LoadState::Idle,
            mod_versions_key: None,
            mod_version_selected: None,
            download_instance_picker: false,
        };
        state.saved_settings = state.settings_value();
        state.reload();
        state
    }

    pub fn t(&self, key: &str) -> String {
        i18n::text(self.lang, key).to_string()
    }

    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        i18n::text_args(self.lang, key, args).to_string()
    }

    fn memory_value(&self, text: &str) -> Option<u32> {
        non_empty(text.trim().to_string()).and_then(|value| value.parse().ok())
    }

    fn settings_value(&self) -> serde_json::Value {
        let mut value = serde_json::json!({
            "game_dir": self.dir.trim(),
            "memory": self.memory_value(&self.memory).unwrap_or(4096),
            "java": self.java.trim(),
            "mirror": self.mirror.trim(),
            "maven": self.maven.trim(),
            "isolate": self.isolate,
            "experimental_ui": self.experimental_ui,
        });
        if let Some(language) = &self.language {
            value["language"] = serde_json::json!(language);
        }
        value["user_java"] = serde_json::json!(crate::java::user_paths());
        value
    }

    fn reload(&mut self) {
        self.instances = list_instances(self.dir.trim());
        self.accounts = crate::microsoft::load_accounts();
        let current = crate::microsoft::current_account();
        self.account_offline = current.as_ref().map(|a| a.offline).unwrap_or(false);
        self.account = current.map(|account| account.name);
    }

    pub fn save_settings(&mut self) {
        let value = self.settings_value();
        if value == self.saved_settings {
            return;
        }
        if write_json(&crate::gui::config_path(), &value, self.lang).is_ok() {
            self.saved_settings = value;
        }
        self.reload();
    }

    pub fn select_language(&mut self, lang: Lang) {
        if self.lang == lang {
            return;
        }
        self.lang = lang;
        self.language = Some(lang.tag().to_string());
        let value = self.settings_value();
        if write_json(&crate::gui::config_path(), &value, lang).is_ok() {
            self.saved_settings = value;
        }
    }

    fn running_task_for(&self, name: &str) -> Option<usize> {
        self.tasks
            .iter()
            .find(|task| task.state == TaskState::Running && task.instance.as_deref() == Some(name))
            .map(|task| task.id)
    }

    fn running_launch_for(&self, name: &str) -> Option<usize> {
        self.tasks
            .iter()
            .find(|task| {
                task.state == TaskState::Running
                    && task.command == "launch"
                    && task.instance.as_deref() == Some(name)
            })
            .map(|task| task.id)
    }

    fn running_task_with_key(&self, key: &str) -> Option<usize> {
        self.tasks
            .iter()
            .find(|task| task.state == TaskState::Running && task.key == key)
            .map(|task| task.id)
    }

    fn search_type(&self) -> &'static str {
        match self.download_tab {
            1 => "modpack",
            3 => "resourcepack",
            4 => "shader",
            5 => "datapack",
            _ => "mod",
        }
    }

    fn cfg_current(&self) -> LaunchConfig {
        LaunchConfig {
            memory: self.memory_value(&self.cfg_memory),
            java: non_empty(self.cfg_java.clone()),
            mirror: non_empty(self.cfg_mirror.clone()),
            maven: non_empty(self.cfg_maven.clone()),
            isolate: if self.cfg_isolate_local {
                Some(self.cfg_isolate)
            } else {
                None
            },
        }
    }

    pub fn filter_versions(&self) -> Vec<GameVersion> {
        let query = self.install_version_query.trim().to_ascii_lowercase();
        self.install_versions
            .iter()
            .filter(|version| {
                query.is_empty()
                    || version.id.to_ascii_lowercase().contains(&query)
                    || version.kind.to_ascii_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    pub fn filter_loader_versions(&self) -> Vec<String> {
        let query = self.install_loader_query.trim().to_ascii_lowercase();
        self.install_loader_versions
            .iter()
            .filter(|version| query.is_empty() || version.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    pub fn filtered_mod_versions(&self) -> Vec<modrinth::ProjectVersion> {
        if self.download_tab == 1 {
            return self.mod_versions.clone();
        }
        let mut loaders = if matches!(self.download_tab, 2 | 4) {
            split_values(&self.mod_loader)
        } else {
            Vec::new()
        };
        let row = self
            .instances
            .iter()
            .find(|row| row.name.as_ref() == self.mod_instance.as_str());
        let mc_version = row.map(|row| row.version.to_string());
        if loaders.is_empty()
            && self.download_tab == 2
            && let Some(row) = row
            && row.loader.as_ref() != "vanilla"
        {
            loaders = vec![row.loader.to_string()];
        }

        self.mod_versions
            .iter()
            .filter(|version| {
                mc_version
                    .as_ref()
                    .is_none_or(|mc| version.game_versions.iter().any(|item| item == mc))
                    && (loaders.is_empty()
                        || version.loaders.iter().any(|item| loaders.contains(item)))
            })
            .cloned()
            .collect()
    }

    pub fn effective_mod_version(&self) -> Option<String> {
        if self.mod_detail.is_none() {
            return None;
        }
        let filtered = self.filtered_mod_versions();
        if let Some(selected) = &self.mod_version_selected
            && filtered.iter().any(|version| &version.id == selected)
        {
            return Some(selected.clone());
        }
        filtered.first().map(|version| version.id.clone())
    }

    pub fn version_kind(&self, kind: &str) -> String {
        match kind {
            "release" => self.t("version.type.release"),
            "snapshot" => self.t("version.type.snapshot"),
            "old_beta" => self.t("version.type.old_beta"),
            "old_alpha" => self.t("version.type.old_alpha"),
            other => other.to_string(),
        }
    }
}

#[component]
fn app() -> Element {
    let state = use_signal(State::new);
    let nav_system = state.read().t("nav.section.system");
    let version = env!("CARGO_PKG_VERSION");
    let running = state
        .read()
        .tasks
        .iter()
        .filter(|task| task.state == TaskState::Running)
        .count();

    rsx! {
        style { dangerous_inner_html: CSS }
        div { class: "app",
            aside { class: "sidebar",
                {widgets::nav_item(state, 0, widgets::NavigationIcon::Dashboard, "nav.instances")}
                {widgets::nav_item(state, 1, widgets::NavigationIcon::Download, "nav.download")}
                div { class: "nav-section", "{nav_system}" }
                {widgets::nav_item_with_badge(
                    state,
                    3,
                    widgets::NavigationIcon::Tasks,
                    "nav.tasks",
                    running,
                )}
                {widgets::nav_item(state, 4, widgets::NavigationIcon::User, "nav.accounts")}
                {widgets::nav_item(state, 2, widgets::NavigationIcon::Settings, "nav.settings")}
                div { class: "spacer" }
                div { class: "version", "{version}" }
            }
            main { class: "content",
                div { class: "page",
                    {views::page(state)}
                }
            }
        }
    }
}

// ---------- 后台任务与界面动作 ----------

fn background<T, F>(work: F) -> smol::channel::Receiver<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = smol::channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(work());
    });
    rx
}

fn spawn_apply<T, F>(rx: smol::channel::Receiver<T>, mut apply: F)
where
    T: Send + 'static,
    F: FnMut(T) + 'static,
{
    spawn(async move {
        if let Ok(value) = rx.recv().await {
            apply(value);
        }
    });
}

fn push_task_log(mut state: Signal<State>, id: usize, line: String) {
    let mut current = state.write();
    let mut notify = false;
    if let Some(task) = current.tasks.iter_mut().find(|task| task.id == id) {
        task.log_lines.push_back(line);
        while task.log_lines.len() > 2_000 {
            task.log_lines.pop_front();
        }
        let now = Instant::now();
        if now.duration_since(task.last_log_notify) >= std::time::Duration::from_millis(50) {
            task.last_log_notify = now;
            notify = true;
        }
    }
    if notify {
        current.task_log_follow = true;
    }
}

fn start_task(
    mut state: Signal<State>,
    title: String,
    instance_name: Option<String>,
    args: Vec<String>,
    refresh: bool,
    save: Option<(String, String, LaunchConfig)>,
) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let lang = state.read().lang;
    let handle = Arc::new(TaskHandle {
        child: std::sync::Mutex::new(None),
        cancelled: std::sync::atomic::AtomicBool::new(false),
    });
    let id = {
        let mut current = state.write();
        let id = current.next_task_id;
        current.next_task_id += 1;
        let command = args.first().cloned().unwrap_or_default();
        let key = args.join("\u{1f}");
        current.tasks.push(TaskEntry {
            id,
            title,
            instance: instance_name,
            command,
            key,
            state: TaskState::Running,
            progress: None,
            log_lines: std::collections::VecDeque::new(),
            last_log_notify: Instant::now(),
            handle: handle.clone(),
        });
        if current.active_task.is_none() {
            current.active_task = Some(id);
        }
        current.task_log_follow = true;
        id
    };

    let (event_tx, event_rx) = smol::channel::unbounded::<TaskEvent>();
    std::thread::spawn(move || {
        execute(exe, args, event_tx, handle, lang);
    });

    spawn(async move {
        let mut success = false;
        while let Ok(event) = event_rx.recv().await {
            match event {
                TaskEvent::Log(line) => push_task_log(state, id, line),
                TaskEvent::Progress { done, total } => {
                    let value = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    let mut current = state.write();
                    if let Some(task) = current.tasks.iter_mut().find(|task| task.id == id) {
                        task.progress = Some(value.clamp(0.0, 1.0));
                    }
                }
                TaskEvent::Done(done) => success = done,
            }
        }

        let mut current = state.write();
        let cancelled = current
            .tasks
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.handle.cancelled.load(Ordering::SeqCst))
            .unwrap_or(false);
        if let Some(task) = current.tasks.iter_mut().find(|task| task.id == id) {
            task.state = if cancelled {
                TaskState::Cancelled
            } else if success {
                TaskState::Success
            } else {
                TaskState::Failed
            };
            task.progress = None;
        }
        if success
            && let Some((dir, name, config)) = save
            && let Err(error) = instance::write_launch_config(&dir, &name, &config)
        {
            let text = current.t_args("error.save_instance", &[("error", &error.to_string())]);
            if let Some(task) = current.tasks.iter_mut().find(|task| task.id == id) {
                task.log_lines.push_back(text);
            }
        }
        let mut refresh_mods = false;
        if refresh {
            current.reload();
            let task_instance = current
                .tasks
                .iter()
                .find(|task| task.id == id)
                .and_then(|task| task.instance.clone());
            refresh_mods =
                task_instance.is_some() && task_instance == Some(current.cfg_name.clone());
        }
        let cfg_name = current.cfg_name.clone();
        drop(current);
        if refresh_mods {
            load_mods(state, cfg_name);
        }
    });
}

fn cancel_task(mut state: Signal<State>, id: usize) {
    let current = state.write();
    let Some(task) = current.tasks.iter().find(|task| task.id == id) else {
        return;
    };
    if task.state != TaskState::Running {
        return;
    }
    task.handle.cancelled.store(true, Ordering::SeqCst);
    if let Some(child) = task.handle.child.lock().unwrap().take() {
        std::thread::spawn(move || kill_tree(child));
    }
}

fn effective_launch(state: &State, name: &str) -> (String, LaunchParams) {
    let dir = state.dir.trim().to_string();
    let config = instance::read_launch_config(&dir, name);
    let params = LaunchParams {
        memory: config.memory.or_else(|| state.memory_value(&state.memory)),
        java: config.java.unwrap_or_else(|| state.java.trim().to_string()),
        mirror: config
            .mirror
            .unwrap_or_else(|| state.mirror.trim().to_string()),
        maven: config
            .maven
            .unwrap_or_else(|| state.maven.trim().to_string()),
        isolate: instance::is_isolated(&dir, name),
    };
    (dir, params)
}

struct LaunchParams {
    memory: Option<u32>,
    java: String,
    mirror: String,
    maven: String,
    isolate: bool,
}

fn launch_instance(mut state: Signal<State>, name: &str) {
    {
        let current = state.read();
        if let Some(id) = current.running_launch_for(name) {
            drop(current);
            state.write().active_task = Some(id);
            return;
        }
    }
    let (dir, params, offline, account) = {
        let current = state.read();
        if current.dir.trim().is_empty() {
            return;
        }
        let (dir, params) = effective_launch(&current, name);
        (
            dir,
            params,
            current.account_offline,
            current.account.clone(),
        )
    };
    let mut args = vec![
        String::from("launch"),
        String::from("--dir"),
        dir,
        String::from("--name"),
        name.to_string(),
        String::from("--memory"),
        params.memory.unwrap_or(4096).to_string(),
    ];
    if offline {
        args.push(String::from("--username"));
        args.push(account.unwrap_or_default());
    }
    if params.isolate {
        args.push(String::from("--isolate"));
    }
    push_option(&mut args, "--java", &params.java);
    push_option(&mut args, "--mirror", &params.mirror);
    push_option(&mut args, "--maven", &params.maven);

    let title = state.read().t_args("task.title.launch", &[("name", name)]);
    start_task(state, title, Some(name.to_string()), args, false, None);
}

fn login_microsoft(mut state: Signal<State>) {
    let running = { state.read().running_task_for(CONFIG_TASK) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let title = state.read().t("task.title.login");
    start_task(
        state,
        title,
        Some(String::from(CONFIG_TASK)),
        vec![String::from("login")],
        true,
        None,
    );
}

fn switch_account(mut state: Signal<State>, name: String, key: String) {
    let running = { state.read().running_task_for(CONFIG_TASK) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let title = state
        .read()
        .t_args("task.title.account", &[("name", &name)]);
    start_task(
        state,
        title,
        Some(String::from(CONFIG_TASK)),
        vec![String::from("account"), String::from("use"), key],
        true,
        None,
    );
}

fn remove_account(mut state: Signal<State>, name: String, key: String) {
    let running = { state.read().running_task_for(CONFIG_TASK) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let title = state
        .read()
        .t_args("task.title.account.remove", &[("name", &name)]);
    start_task(
        state,
        title,
        Some(String::from(CONFIG_TASK)),
        vec![String::from("account"), String::from("remove"), key],
        true,
        None,
    );
}

fn add_offline_account(mut state: Signal<State>) {
    let name = state.read().offline_name.trim().to_string();
    if name.is_empty() {
        return;
    }
    let running = { state.read().running_task_for(CONFIG_TASK) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    {
        let mut current = state.write();
        current.offline_name.clear();
        current.offline_form_open = false;
    }
    let title = state
        .read()
        .t_args("task.title.account.add", &[("name", &name)]);
    start_task(
        state,
        title,
        Some(String::from(CONFIG_TASK)),
        vec![String::from("account"), String::from("add-offline"), name],
        true,
        None,
    );
}

fn load_mods(state: Signal<State>, name: String) {
    let dir = state.read().dir.trim().to_string();
    let instance_name = name.clone();
    spawn_apply(list_mods(dir, instance_name), move |entries| {
        let mut state = state;
        let mut current = state.write();
        if current.cfg_name == name {
            current.mods = entries;
        }
    });
}

fn list_mods(dir: String, instance_name: String) -> smol::channel::Receiver<Vec<mods::ModEntry>> {
    background(move || mods::list(&dir, &instance_name).unwrap_or_default())
}

fn toggle_mod(mut state: Signal<State>, file: String) {
    let (dir, name) = {
        let current = state.read();
        (current.dir.trim().to_string(), current.cfg_name.clone())
    };
    if dir.is_empty() || name.is_empty() {
        return;
    }
    let running = { state.read().running_task_for(&name) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let args = vec![
        String::from("mods"),
        String::from("toggle"),
        String::from("--dir"),
        dir,
        String::from("--name"),
        name.clone(),
        String::from("--mod"),
        file,
    ];
    let title = state
        .read()
        .t_args("task.title.mods_toggle", &[("name", &name)]);
    start_task(state, title, Some(name), args, true, None);
}

fn remove_mod(mut state: Signal<State>, file: String) {
    let (dir, name) = {
        let current = state.read();
        (current.dir.trim().to_string(), current.cfg_name.clone())
    };
    if dir.is_empty() || name.is_empty() {
        return;
    }
    let running = { state.read().running_task_for(&name) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let args = vec![
        String::from("mods"),
        String::from("remove"),
        String::from("--dir"),
        dir,
        String::from("--name"),
        name.clone(),
        String::from("--mod"),
        file,
    ];
    let title = state
        .read()
        .t_args("task.title.mods_remove", &[("name", &name)]);
    start_task(state, title, Some(name), args, true, None);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PathTarget {
    GameDir,
    Java,
    InstanceJava,
    InstallJava,
    ImportFile,
    ScanJavaDir,
}

fn pick_path(mut state: Signal<State>, target: PathTarget, title: String) {
    let picked = background(move || {
        let dialog = rfd::FileDialog::new().set_title(&title);
        if target == PathTarget::ImportFile {
            dialog.pick_file()
        } else {
            dialog.pick_folder()
        }
    });
    spawn_apply(picked, move |picked| {
        let Some(path) = picked else {
            return;
        };
        let value = path.to_string_lossy().to_string();
        if target == PathTarget::ScanJavaDir {
            if crate::java::add_user_path(&value).is_err() {
                return;
            }
            detect_java(state);
            return;
        }
        let mut current = state.write();
        match target {
            PathTarget::GameDir => current.dir = value,
            PathTarget::Java => current.java = value,
            PathTarget::InstanceJava => current.cfg_java = value,
            PathTarget::InstallJava => current.install_java = value,
            PathTarget::ImportFile => current.import_file = value,
            PathTarget::ScanJavaDir => {}
        }
    });
}

fn open_folder(state: Signal<State>, name: String) {
    let dir = state.read().dir.trim().to_string();
    let path = Path::new(&dir).join("versions").join(&name);
    if path.is_dir() {
        let _ = open_directory(&path);
    }
}

fn back_to_instances(mut state: Signal<State>) {
    let mut current = state.write();
    current.instance_page = 0;
    current.cfg_name.clear();
}

fn install_version(mut state: Signal<State>) {
    let (dir, version, name, loader, loader_version, java, mirror, maven, memory, isolate) = {
        let current = state.read();
        (
            current.dir.trim().to_string(),
            current.install_version.trim().to_string(),
            current.install_name.trim().to_string(),
            current.install_loader.trim().to_string(),
            current.install_loader_version.trim().to_string(),
            current.install_java.trim().to_string(),
            current.install_mirror.trim().to_string(),
            current.install_maven.trim().to_string(),
            current.memory_value(&current.install_memory),
            current.install_isolate,
        )
    };
    if dir.is_empty() || version.is_empty() {
        return;
    }

    let mut args = vec![
        String::from("download"),
        String::from("--dir"),
        dir.clone(),
        String::from("--version"),
        version.clone(),
    ];
    push_option(&mut args, "--name", &name);
    push_option(&mut args, "--loader", &loader);
    push_option(&mut args, "--loader-version", &loader_version);
    push_option(&mut args, "--java", &java);
    push_option(&mut args, "--mirror", &mirror);
    push_option(&mut args, "--maven", &maven);

    let key = args.join("\u{1f}");
    let running = { state.read().running_task_with_key(&key) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }

    let instance_name = (!name.is_empty()).then(|| name.clone());
    let save = (!name.is_empty()).then(|| {
        (
            dir,
            name,
            LaunchConfig {
                memory,
                java: Some(java),
                mirror: Some(mirror),
                maven: Some(maven),
                isolate: Some(isolate),
            },
        )
    });
    let title = state
        .read()
        .t_args("task.title.install", &[("version", &version)]);
    start_task(state, title, instance_name, args, true, save);
}

fn load_game_versions(mut state: Signal<State>) {
    if matches!(state.read().install_versions_state, LoadState::Loading) {
        return;
    }
    let (mirror, maven) = {
        let mut current = state.write();
        current.install_versions_state = LoadState::Loading;
        (
            current.install_mirror.trim().to_string(),
            current.install_maven.trim().to_string(),
        )
    };
    let rx = background(move || {
        let origins = crate::download::Origins {
            mojang: (!mirror.is_empty()).then_some(mirror.as_str()),
            maven: (!maven.is_empty()).then_some(maven.as_str()),
        };
        catalog::game_versions(&origins)
    });
    spawn_apply(rx, move |result| {
        let mut current = state.write();
        match result {
            Ok(versions) => {
                current.install_versions = versions;
                current.install_versions_state = LoadState::Ready;
            }
            Err(error) => current.install_versions_state = LoadState::Failed(error),
        }
    });
}

fn load_loader_versions(mut state: Signal<State>) {
    let (loader, version) = {
        let current = state.read();
        (
            current.install_loader.trim().to_string(),
            current.install_version.trim().to_string(),
        )
    };
    if loader.is_empty() || version.is_empty() {
        let mut current = state.write();
        current.install_loader_versions.clear();
        current.install_loader_versions_key = None;
        current.install_loader_versions_state = LoadState::Idle;
        return;
    }
    let key = (loader.clone(), version.clone());
    let (mirror, maven) = {
        let mut current = state.write();
        if current.install_loader_versions_key.as_ref() == Some(&key)
            && matches!(
                current.install_loader_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            return;
        }
        current.install_loader_versions.clear();
        current.install_loader_versions_key = Some(key.clone());
        current.install_loader_versions_state = LoadState::Loading;
        (
            current.install_mirror.trim().to_string(),
            current.install_maven.trim().to_string(),
        )
    };
    let rx = background(move || {
        let origins = crate::download::Origins {
            mojang: (!mirror.is_empty()).then_some(mirror.as_str()),
            maven: (!maven.is_empty()).then_some(maven.as_str()),
        };
        catalog::loader_versions(&loader, &version, &origins)
    });
    spawn_apply(rx, move |result| {
        let mut current = state.write();
        if current.install_loader_versions_key.as_ref() != Some(&key) {
            return;
        }
        match result {
            Ok(versions) => {
                current.install_loader_versions = versions;
                current.install_loader_versions_state = LoadState::Ready;
            }
            Err(error) => current.install_loader_versions_state = LoadState::Failed(error),
        }
    });
}

fn import_pack(state: Signal<State>) {
    let (dir, file, name, java, mirror, maven, optional) = {
        let current = state.read();
        (
            current.dir.trim().to_string(),
            current.import_file.trim().to_string(),
            current.import_name.trim().to_string(),
            current.java.trim().to_string(),
            current.mirror.trim().to_string(),
            current.maven.trim().to_string(),
            current.import_optional,
        )
    };
    if dir.is_empty() || file.is_empty() {
        return;
    }
    let mut args = vec![
        String::from("import"),
        String::from("--dir"),
        dir,
        String::from("--file"),
        file,
    ];
    push_option(&mut args, "--name", &name);
    push_option(&mut args, "--java", &java);
    push_option(&mut args, "--mirror", &mirror);
    push_option(&mut args, "--maven", &maven);
    if optional {
        args.push(String::from("--with-optional"));
    }
    let instance_name = (!name.is_empty()).then(|| name.clone());
    let title = state.read().t("task.title.import");
    start_task(state, title, instance_name, args, true, None);
}

fn import_modrinth(mut state: Signal<State>) {
    let (dir, slug, name, java, mirror, maven, optional, version) = {
        let current = state.read();
        let slug = current
            .mod_detail
            .as_ref()
            .map(|detail| detail.slug.clone())
            .unwrap_or_else(|| current.mod_slug.trim().to_string());
        (
            current.dir.trim().to_string(),
            slug,
            current.import_name.trim().to_string(),
            current.java.trim().to_string(),
            current.mirror.trim().to_string(),
            current.maven.trim().to_string(),
            current.import_optional,
            current.effective_mod_version(),
        )
    };
    if dir.is_empty() || slug.is_empty() {
        return;
    }
    let mut args = vec![
        String::from("import"),
        String::from("--dir"),
        dir,
        String::from("--modrinth"),
        slug,
    ];
    push_option(&mut args, "--name", &name);
    push_option(&mut args, "--java", &java);
    push_option(&mut args, "--mirror", &mirror);
    push_option(&mut args, "--maven", &maven);
    if let Some(version) = version {
        args.push(String::from("--pack-version"));
        args.push(version);
    }
    if optional {
        args.push(String::from("--with-optional"));
    }
    let key = args.join("\u{1f}");
    let running = { state.read().running_task_with_key(&key) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let instance_name = (!name.is_empty()).then(|| name.clone());
    let title = state.read().t("task.title.import");
    start_task(state, title, instance_name, args, true, None);
}

fn search_mods(mut state: Signal<State>) {
    let (query, project_type, loaders, versions, limit, key) = {
        let current = state.read();
        if current.mod_query.trim().is_empty()
            || matches!(current.search_results_state, LoadState::Loading)
        {
            return;
        }
        let query = current.mod_query.trim().to_string();
        let project_type = String::from(current.search_type());
        let loaders: Vec<String> = if matches!(current.download_tab, 2 | 4) {
            split_values(&current.mod_loader)
        } else {
            Vec::new()
        };
        let versions = if project_type == "modpack" {
            split_values(&current.mod_version)
        } else {
            current
                .instances
                .iter()
                .find(|row| row.name.as_ref() == current.mod_instance.as_str())
                .map(|row| vec![row.version.to_string()])
                .unwrap_or_default()
        };
        let limit: u32 = current.mod_limit.trim().parse().unwrap_or(10);
        let key = format!(
            "{query}\u{1f}{project_type}\u{1f}{}\u{1f}{}\u{1f}{limit}",
            loaders.join(","),
            versions.join(",")
        );
        (query, project_type, loaders, versions, limit, key)
    };
    {
        let mut current = state.write();
        current.search_results.clear();
        current.search_icons.clear();
        current.search_results_key = Some(key.clone());
        current.search_results_state = LoadState::Loading;
    }
    let rx =
        background(move || modrinth::search(&query, &project_type, &loaders, &versions, limit));
    spawn_apply(rx, move |result| {
        {
            let mut current = state.write();
            if current.search_results_key.as_ref() != Some(&key) {
                return;
            }
            match result {
                Ok(hits) => {
                    current.search_results = hits;
                    current.search_results_state = LoadState::Ready;
                }
                Err(error) => current.search_results_state = LoadState::Failed(error),
            }
        }
        load_search_icons(state);
    });
}

fn load_search_icons(mut state: Signal<State>) {
    let pending: Vec<(String, String)> = {
        let current = state.read();
        current
            .search_results
            .iter()
            .filter(|hit| !hit.icon_url.is_empty() && !current.search_icons.contains_key(&hit.slug))
            .map(|hit| (hit.slug.clone(), hit.icon_url.clone()))
            .collect()
    };
    for (slug, url) in pending {
        let fetch = url.clone();
        let rx = background(move || crate::download::get_bytes(&fetch).ok());
        spawn_apply(rx, move |bytes| {
            let Some(bytes) = bytes else {
                return;
            };
            let uri = data_uri(&url, &bytes);
            state.write().search_icons.insert(slug.clone(), uri);
        });
    }
}

fn open_mod_detail(mut state: Signal<State>, hit: modrinth::Hit) {
    let slug = hit.slug.clone();
    {
        let mut current = state.write();
        current.mod_detail = Some(hit);
        current.download_instance_picker = false;
    }
    load_mod_versions(state, slug);
}

fn load_mod_versions(mut state: Signal<State>, slug: String) {
    {
        let mut current = state.write();
        if current.mod_versions_key.as_deref() == Some(slug.as_str())
            && matches!(
                current.mod_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            return;
        }
        current.mod_versions.clear();
        current.mod_version_selected = None;
        current.mod_versions_key = Some(slug.clone());
        current.mod_versions_state = LoadState::Loading;
    }
    let fetch = slug.clone();
    let rx = background(move || modrinth::project_versions(&fetch));
    spawn_apply(rx, move |result| {
        let mut current = state.write();
        if current.mod_versions_key.as_deref() != Some(slug.as_str()) {
            return;
        }
        match result {
            Ok(list) => {
                current.mod_versions = list;
                current.mod_versions_state = LoadState::Ready;
            }
            Err(error) => current.mod_versions_state = LoadState::Failed(error),
        }
    });
}

fn add_mod(mut state: Signal<State>) {
    let (dir, name, slug, project_type, version) = {
        let current = state.read();
        let Some(detail) = current.mod_detail.clone() else {
            return;
        };
        (
            current.dir.trim().to_string(),
            current.mod_instance.trim().to_string(),
            detail.slug,
            String::from(current.search_type()),
            current.effective_mod_version(),
        )
    };
    if dir.is_empty() || name.is_empty() || slug.is_empty() {
        return;
    }
    let mut args = vec![
        String::from("add"),
        String::from("--dir"),
        dir,
        String::from("--name"),
        name.clone(),
        String::from("--slug"),
        slug.clone(),
        String::from("--type"),
        project_type,
    ];
    if let Some(version) = version {
        args.push(String::from("--version"));
        args.push(version);
    }
    let key = args.join("\u{1f}");
    let running = { state.read().running_task_with_key(&key) };
    if let Some(id) = running {
        state.write().active_task = Some(id);
        return;
    }
    let title = state.read().t_args("task.title.add", &[("slug", &slug)]);
    start_task(state, title, Some(name), args, false, None);
}

fn open_instance_settings(mut state: Signal<State>, name: String) {
    let (dir, config, required) = {
        let current = state.read();
        let dir = current.dir.trim().to_string();
        if dir.is_empty() {
            return;
        }
        let config = instance::read_launch_config(&dir, &name);
        let required = crate::version::resolve(&dir, &name)
            .ok()
            .map(|resolved| crate::java::required_major_for_version(&resolved.json));
        (dir, config, required)
    };
    {
        let mut current = state.write();
        current.cfg_name = name.clone();
        current.cfg_memory = config
            .memory
            .map(|value| value.to_string())
            .unwrap_or_default();
        current.cfg_java = config.java.clone().unwrap_or_default();
        current.cfg_mirror = config.mirror.clone().unwrap_or_default();
        current.cfg_maven = config.maven.clone().unwrap_or_default();
        current.cfg_isolate = instance::is_isolated(&dir, &name);
        current.cfg_isolate_local = config.isolate.is_some();
        current.cfg_java_required = required;
        current.cfg_java_note = None;
        current.cfg_saved = config;
        current.mods = Vec::new();
        current.instance_page = 2;
    }
    load_mods(state, name);
}

fn save_instance_settings(mut state: Signal<State>) {
    let (dir, name, config) = {
        let current = state.read();
        (
            current.dir.trim().to_string(),
            current.cfg_name.clone(),
            current.cfg_current(),
        )
    };
    if dir.is_empty() || name.is_empty() {
        return;
    }
    let config = LaunchConfig {
        isolate: Some(state.read().cfg_isolate),
        ..config
    };
    let _ = instance::write_launch_config(&dir, &name, &config);
    {
        let mut current = state.write();
        current.cfg_isolate_local = true;
        current.cfg_saved = config;
    }
    load_mods(state, name);
    state.write().reload();
}

fn detect_java(mut state: Signal<State>) {
    {
        let mut current = state.write();
        current.java_runtimes_state = LoadState::Loading;
        current.java_picker = true;
    }
    let rx = background(|| crate::java::detect(true));
    spawn_apply(rx, move |runtimes| {
        let mut current = state.write();
        current.java_runtimes = runtimes;
        current.java_runtimes_state = LoadState::Ready;
    });
}

fn pick_java_runtime(mut state: Signal<State>, index: usize) {
    let path = {
        let current = state.read();
        current
            .java_runtimes
            .get(index)
            .map(|runtime| runtime.path.clone())
    };
    let Some(path) = path else {
        return;
    };
    {
        let mut current = state.write();
        current.java = path;
        current.java_picker = false;
        current.save_settings();
    }
}

fn auto_select_instance_java(mut state: Signal<State>) {
    let Some(required) = state.read().cfg_java_required else {
        return;
    };
    let rx = background(move || {
        let runtimes = crate::java::detect(false);
        crate::java::select(&runtimes, required).map(|runtime| runtime.path.clone())
    });
    spawn_apply(rx, move |picked| {
        let mut current = state.write();
        match picked {
            Some(path) => {
                current.cfg_java = path;
                current.cfg_java_note = None;
            }
            None => {
                let major = required.to_string();
                current.cfg_java_note =
                    Some(current.t_args("settings.java.auto_failed", &[("major", &major)]));
            }
        }
    });
}

// ---------- 缩略图 data: URI ----------

fn data_uri(url: &str, bytes: &[u8]) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

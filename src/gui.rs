use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, AppContext, Bounds, Context, CursorStyle, Div, ElementId, Entity, Image,
    ImageFormat, IntoElement, ObjectFit, ParentElement, PathPromptOptions, Render, ScrollHandle,
    SharedString, Stateful, Styled, Window, WindowBounds, WindowOptions, div, img,
    prelude::*, px, size,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Root, Sizable, StyledExt, TitleBar, h_flex, input,
    switch::Switch,
    theme::{Theme, ThemeMode},
    v_flex,
};
use gpui_kit_assets::Assets;
use gpui_platform::application;
use serde_json::{Value, json};
use smol::channel;

use crate::catalog::{self, GameVersion};
use crate::i18n::{self, Lang};
use crate::instance::{self, LaunchConfig};
use crate::modrinth;
use crate::mods;

mod skin {
    use gpui::{Rgba, rgb};

    pub const WINDOW: u32 = 0x0f0f0f;
    pub const TITLEBAR_LINE: u32 = 0x1f1f1f;
    pub const PANEL: u32 = 0x242424;
    pub const SIDEBAR_TEXT: u32 = 0xd0d0d0;
    pub const CONTENT: u32 = 0x161616;
    pub const CARD: u32 = 0x242424;
    pub const DIVIDER: u32 = 0x2f2f2f;
    pub const HOVER: u32 = 0x2e2e2e;
    pub const ACTIVE: u32 = 0x3a3a3a;
    pub const FIELD: u32 = 0x2f2f2f;
    pub const FIELD_BORDER: u32 = 0x3f3f3f;
    pub const TEXT: u32 = 0xfafafa;
    pub const MUTED: u32 = 0xa3a3a3;
    pub const FAINT: u32 = 0x757575;
    pub const ACCENT: u32 = 0xfafafa;
    pub const ACCENT_HOVER: u32 = 0xffffff;
    pub const ACCENT_ACTIVE: u32 = 0xe4e4e4;
    pub const ACCENT_FG: u32 = 0x121212;
    pub const BUSY: u32 = 0x7aa2f7;
    pub const OK: u32 = 0x4ade80;
    pub const DANGER: u32 = 0x7f1d1d;
    pub const DANGER_HOVER: u32 = 0x991b1b;
    pub const DANGER_FG: u32 = 0xfecaca;

    pub fn c(hex: u32) -> Rgba {
        rgb(hex)
    }
}

fn apply_skin(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.radius = px(8.);
        theme.radius_lg = px(12.);
        theme.focus_ring = false;
        let colors = &mut theme.colors;
        colors.background = skin::c(skin::CONTENT).into();
        colors.foreground = skin::c(skin::TEXT).into();
        colors.border = skin::c(skin::DIVIDER).into();
        colors.muted = skin::c(skin::FIELD).into();
        colors.muted_foreground = skin::c(skin::MUTED).into();
        colors.accent = skin::c(skin::ACTIVE).into();
        colors.accent_foreground = skin::c(skin::TEXT).into();
        colors.title_bar = skin::c(skin::WINDOW).into();
        colors.sidebar = skin::c(skin::PANEL).into();
        colors.sidebar_foreground = skin::c(skin::SIDEBAR_TEXT).into();
        colors.sidebar_accent = skin::c(skin::ACTIVE).into();
        colors.sidebar_accent_foreground = skin::c(skin::TEXT).into();
        colors.sidebar_border = skin::c(skin::PANEL).into();
        colors.sidebar_primary = skin::c(skin::ACCENT).into();
        colors.sidebar_primary_foreground = skin::c(skin::ACCENT_FG).into();
        colors.list = skin::c(skin::CARD).into();
        colors.list_hover = skin::c(skin::HOVER).into();
        colors.list_active = skin::c(skin::ACTIVE).into();
        colors.list_active_border = skin::c(skin::DIVIDER).into();
        colors.list_head = skin::c(skin::CARD).into();
        colors.input = skin::c(skin::FIELD_BORDER).into();
        colors.popover = skin::c(skin::CARD).into();
        colors.popover_foreground = skin::c(skin::TEXT).into();
        colors.primary = skin::c(skin::ACCENT).into();
        colors.primary_hover = skin::c(skin::ACCENT_HOVER).into();
        colors.primary_active = skin::c(skin::ACCENT_ACTIVE).into();
        colors.primary_foreground = skin::c(skin::ACCENT_FG).into();
        colors.button = skin::c(skin::FIELD).into();
        colors.button_hover = skin::c(skin::HOVER).into();
        colors.button_active = skin::c(skin::ACTIVE).into();
        colors.button_foreground = skin::c(skin::TEXT).into();
        colors.button_primary = skin::c(skin::ACCENT).into();
        colors.button_primary_hover = skin::c(skin::ACCENT_HOVER).into();
        colors.button_primary_active = skin::c(skin::ACCENT_ACTIVE).into();
        colors.button_primary_foreground = skin::c(skin::ACCENT_FG).into();
        colors.button_secondary = skin::c(skin::FIELD).into();
        colors.button_secondary_hover = skin::c(skin::HOVER).into();
        colors.button_secondary_foreground = skin::c(skin::TEXT).into();
        colors.ring = skin::c(skin::MUTED).into();
        colors.selection = skin::c(skin::ACTIVE).into();
        colors.caret = skin::c(skin::TEXT).into();
        colors.scrollbar_thumb = skin::c(skin::ACTIVE).into();
        colors.scrollbar_thumb_hover = skin::c(skin::MUTED).into();
        colors.switch = skin::c(0x424242).into();
        colors.switch_thumb = skin::c(0xe8e8e8).into();
    });
}

struct TaskHandle {
    child: Mutex<Option<std::process::Child>>,
    cancelled: AtomicBool,
}

enum TaskEvent {
    Log(String),
    Progress { done: usize, total: usize },
    Done(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Clone, PartialEq, Eq)]
enum LoadState {
    Idle,
    Loading,
    Ready,
    Failed(SharedString),
}

fn task_state_key(state: TaskState) -> &'static str {
    match state {
        TaskState::Running => "task.state.running",
        TaskState::Success => "task.state.success",
        TaskState::Failed => "task.state.failed",
        TaskState::Cancelled => "task.state.cancelled",
    }
}

struct TaskEntry {
    id: usize,
    title: SharedString,
    instance: Option<SharedString>,
    state: TaskState,
    progress: Option<f32>,
    log_lines: VecDeque<SharedString>,
    last_log_notify: Instant,
    handle: Arc<TaskHandle>,
}

#[derive(Clone)]
struct InstanceRow {
    name: SharedString,
    version: SharedString,
    loader: SharedString,
    isolated: bool,
}

#[derive(Clone, Copy)]
enum PathTarget {
    GameDir,
    Java,
    InstanceJava,
    InstallJava,
    ImportFile,
    ScanJavaDir,
}

pub struct LauncherUi {
    tab: usize,
    prev_tab: usize,
    instance_page: usize,
    download_tab: usize,
    lang: Lang,
    language: Option<SharedString>,
    saved_settings: Value,
    instances: Vec<InstanceRow>,
    account: Option<SharedString>,
    accounts: Vec<crate::microsoft::MinecraftAccount>,
    account_offline: bool,
    account_menu_open: bool,
    offline_form_open: bool,
    offline_name_input: Entity<input::InputState>,
    tasks: Vec<TaskEntry>,
    active_task: Option<usize>,
    next_task_id: usize,
    task_log_scroll: ScrollHandle,
    task_log_follow: bool,

    dir_input: Entity<input::InputState>,
    username_input: Entity<input::InputState>,
    memory_input: Entity<input::InputState>,
    java_input: Entity<input::InputState>,
    mirror_input: Entity<input::InputState>,
    maven_input: Entity<input::InputState>,
    isolate: bool,

    install_version_input: Entity<input::InputState>,
    install_name_input: Entity<input::InputState>,
    install_loader_input: Entity<input::InputState>,
    install_loader_version_input: Entity<input::InputState>,
    install_username_input: Entity<input::InputState>,
    install_memory_input: Entity<input::InputState>,
    install_java_input: Entity<input::InputState>,
    install_mirror_input: Entity<input::InputState>,
    install_maven_input: Entity<input::InputState>,
    install_isolate: bool,
    install_versions: Vec<GameVersion>,
    install_versions_state: LoadState,
    install_version_picker: bool,
    install_version_query: Entity<input::InputState>,
    install_loader_versions: Vec<SharedString>,
    install_loader_versions_state: LoadState,
    install_loader_versions_key: Option<(String, String)>,
    install_loader_picker: bool,
    install_loader_query: Entity<input::InputState>,
    download_version_picker: bool,
    download_version_query: Entity<input::InputState>,

    cfg_name: SharedString,
    cfg_username_input: Entity<input::InputState>,
    cfg_memory_input: Entity<input::InputState>,
    cfg_java_input: Entity<input::InputState>,
    cfg_mirror_input: Entity<input::InputState>,
    cfg_maven_input: Entity<input::InputState>,
    cfg_isolate: bool,
    cfg_isolate_local: bool,
    cfg_java_required: Option<u32>,
    cfg_java_note: Option<SharedString>,
    cfg_saved: LaunchConfig,
    java_runtimes: Vec<crate::java::JavaRuntime>,
    java_runtimes_state: LoadState,
    java_picker: bool,
    mods: Vec<mods::ModEntry>,

    import_file_input: Entity<input::InputState>,
    import_name_input: Entity<input::InputState>,
    import_optional: bool,

    mod_query_input: Entity<input::InputState>,
    mod_loader_input: Entity<input::InputState>,
    mod_version_input: Entity<input::InputState>,
    mod_limit_input: Entity<input::InputState>,
    mod_slug_input: Entity<input::InputState>,
    mod_instance_input: Entity<input::InputState>,
    search_results: Vec<modrinth::Hit>,
    search_results_state: LoadState,
    search_results_key: Option<String>,
    search_icons: HashMap<String, Arc<Image>>,
    mod_detail: Option<modrinth::Hit>,
    mod_versions: Vec<modrinth::ProjectVersion>,
    mod_versions_state: LoadState,
    mod_versions_key: Option<String>,
    mod_version_selected: Option<String>,
    download_instance_picker: bool,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    application().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        apply_skin(cx);

        let bounds = Bounds::centered(None, size(px(1180.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(980.), px(640.))),
            ..TitleBar::window_options()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| LauncherUi::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap();
        let on_closed = cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        });
        on_closed.detach();
    });
    Ok(())
}

impl LauncherUi {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_settings();
        let memory = settings.memory.unwrap_or(4096).min(65536).to_string();
        let lang = settings
            .language
            .as_deref()
            .and_then(Lang::from_tag)
            .unwrap_or(i18n::FALLBACK);

        let mut this = LauncherUi {
            tab: 0,
            prev_tab: 0,
            instance_page: 0,
            download_tab: 0,
            lang,
            language: settings.language.clone().map(SharedString::from),
            saved_settings: Value::Null,
            instances: Vec::new(),
            account: None,
            accounts: Vec::new(),
            account_offline: false,
            account_menu_open: false,
            offline_form_open: false,
            offline_name_input: input(window, "", cx),
            tasks: Vec::new(),
            active_task: None,
            next_task_id: 1,
            task_log_scroll: ScrollHandle::new(),
            task_log_follow: false,

            dir_input: input(window, &settings.game_dir, cx),
            username_input: input(window, &settings.username, cx),
            memory_input: input(window, &memory, cx),
            java_input: input(window, &settings.java, cx),
            mirror_input: input(window, &settings.mirror, cx),
            maven_input: input(window, &settings.maven, cx),
            isolate: settings.isolate,

            install_version_input: input(window, "1.20.1", cx),
            install_name_input: input(window, "", cx),
            install_loader_input: input(window, "", cx),
            install_loader_version_input: input(window, "", cx),
            install_username_input: input(window, &settings.username, cx),
            install_memory_input: input(window, &memory.clone(), cx),
            install_java_input: input(window, &settings.java.clone(), cx),
            install_mirror_input: input(window, &settings.mirror.clone(), cx),
            install_maven_input: input(window, &settings.maven.clone(), cx),
            install_isolate: settings.isolate,
            install_versions: Vec::new(),
            install_versions_state: LoadState::Idle,
            install_version_picker: false,
            install_version_query: input(window, "", cx),
            install_loader_versions: Vec::new(),
            install_loader_versions_state: LoadState::Idle,
            install_loader_versions_key: None,
            install_loader_picker: false,
            install_loader_query: input(window, "", cx),
            download_version_picker: false,
            download_version_query: input(window, "", cx),

            cfg_name: SharedString::default(),
            cfg_username_input: input(window, "", cx),
            cfg_memory_input: input(window, &memory.clone(), cx),
            cfg_java_input: input(window, &settings.java.clone(), cx),
            cfg_mirror_input: input(window, &settings.mirror.clone(), cx),
            cfg_maven_input: input(window, &settings.maven.clone(), cx),
            cfg_isolate: false,
            cfg_isolate_local: false,
            cfg_java_required: None,
            cfg_java_note: None,
            cfg_saved: LaunchConfig::default(),
            java_runtimes: Vec::new(),
            java_runtimes_state: LoadState::Idle,
            java_picker: false,
            mods: Vec::new(),

            import_file_input: input(window, "", cx),
            import_name_input: input(window, "", cx),
            import_optional: false,

            mod_query_input: input(window, "", cx),
            mod_loader_input: input(window, "", cx),
            mod_version_input: input(window, "", cx),
            mod_limit_input: input(window, "10", cx),
            mod_slug_input: input(window, "", cx),
            mod_instance_input: input(window, "", cx),
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
        for watched in [
            &this.install_version_query,
            &this.install_loader_query,
            &this.install_version_input,
            &this.install_loader_input,
        ] {
            cx.observe(watched, |this, _, cx| {
                if this.install_version_picker || this.install_loader_picker {
                    cx.notify();
                }
            })
            .detach();
        }
        cx.observe(&this.download_version_query, |this, _, cx| {
            if this.download_version_picker {
                cx.notify();
            }
        })
        .detach();
        cx.observe(&this.mod_slug_input, |this, _, cx| {
            if !matches!(this.search_results_state, LoadState::Idle) {
                cx.notify();
            }
        })
        .detach();
        cx.observe(&this.mod_instance_input, |_, _, cx| {
            cx.notify();
        })
        .detach();
        for watched in [
            &this.dir_input,
            &this.username_input,
            &this.memory_input,
            &this.java_input,
            &this.mirror_input,
            &this.maven_input,
        ] {
            cx.observe(watched, |_, _, cx| cx.notify()).detach();
        }
        for watched in [
            &this.cfg_username_input,
            &this.cfg_memory_input,
            &this.cfg_java_input,
            &this.cfg_mirror_input,
            &this.cfg_maven_input,
        ] {
            cx.observe(watched, |_, _, cx| cx.notify()).detach();
        }
        this.saved_settings = this.settings_value(cx);
        this.reload(cx);
        this
    }

    fn value(entity: &Entity<input::InputState>, cx: &Context<Self>) -> String {
        entity.read(cx).value().trim().to_string()
    }

    fn set_input(
        entity: &Entity<input::InputState>,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.to_string();
        entity.update(cx, move |state, cx| state.set_value(value, window, cx));
    }

    fn memory_value(&self, entity: &Entity<input::InputState>, cx: &Context<Self>) -> Option<u32> {
        let text = Self::value(entity, cx);
        if text.is_empty() {
            return None;
        }
        text.parse().ok()
    }

    fn t(&self, key: &str) -> SharedString {
        i18n::text(self.lang, key)
    }

    fn t_args(&self, key: &str, args: &[(&str, &str)]) -> SharedString {
        i18n::text_args(self.lang, key, args)
    }

    fn select_language(&mut self, lang: Lang, cx: &mut Context<Self>) {
        if self.lang == lang {
            return;
        }
        self.lang = lang;
        self.language = Some(SharedString::from(lang.tag()));
        let value = self.settings_value(cx);
        if write_json(&config_path(), &value, lang).is_ok() {
            self.saved_settings = value;
        }
        cx.notify();
    }

    fn settings_value(&self, cx: &Context<Self>) -> Value {
        let memory = self.memory_value(&self.memory_input, cx);
        let mut value = json!({
            "game_dir": Self::value(&self.dir_input, cx),
            "username": Self::value(&self.username_input, cx),
            "memory": memory.unwrap_or(4096),
            "java": Self::value(&self.java_input, cx),
            "mirror": Self::value(&self.mirror_input, cx),
            "maven": Self::value(&self.maven_input, cx),
            "isolate": self.isolate,
        });
        if let Some(language) = &self.language {
            value["language"] = json!(language.to_string());
        }
        value["user_java"] = json!(crate::java::user_paths());
        value
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        self.instances = list_instances(&dir);
        self.accounts = crate::microsoft::load_accounts();
        let current = crate::microsoft::current_account();
        self.account_offline = current.as_ref().map(|account| account.offline).unwrap_or(false);
        self.account = current.map(|account| SharedString::from(account.name));
        cx.notify();
    }

    fn push_task_log(&mut self, id: usize, line: String, cx: &mut Context<Self>) {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return;
        };
        task.log_lines.push_back(SharedString::from(line));
        while task.log_lines.len() > 2_000 {
            task.log_lines.pop_front();
        }
        let now = Instant::now();
        if now.duration_since(task.last_log_notify) >= Duration::from_millis(50) {
            task.last_log_notify = now;
            self.task_log_follow = true;
            cx.notify();
        }
    }

    fn running_task_for(&self, name: &str) -> Option<usize> {
        self.tasks
            .iter()
            .find(|task| {
                task.state == TaskState::Running && task.instance.as_deref() == Some(name)
            })
            .map(|task| task.id)
    }

    fn start_task(
        &mut self,
        title: SharedString,
        instance: Option<SharedString>,
        args: Vec<String>,
        refresh: bool,
        save: Option<(String, String, LaunchConfig)>,
        cx: &mut Context<Self>,
    ) {
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(_) => return,
        };
        let id = self.next_task_id;
        self.next_task_id += 1;
        let handle = Arc::new(TaskHandle {
            child: Mutex::new(None),
            cancelled: AtomicBool::new(false),
        });
        self.tasks.push(TaskEntry {
            id,
            title,
            instance,
            state: TaskState::Running,
            progress: None,
            log_lines: VecDeque::new(),
            last_log_notify: Instant::now(),
            handle: handle.clone(),
        });
        if self.active_task.is_none() {
            self.active_task = Some(id);
        }
        self.task_log_follow = true;
        cx.notify();

        let (event_tx, event_rx) = channel::unbounded::<TaskEvent>();
        let lang = self.lang;
        std::thread::spawn(move || {
            execute(exe, args, event_tx, handle, lang);
        });

        cx.spawn(async move |weak, cx| {
            let mut success = false;
            while let Ok(event) = event_rx.recv().await {
                let stop = match event {
                    TaskEvent::Log(line) => {
                        weak.update(cx, |this, cx| this.push_task_log(id, line, cx))
                            .ok();
                        false
                    }
                    TaskEvent::Progress { done, total } => {
                        weak.update(cx, |this, cx| {
                            let value = if total == 0 {
                                0.0
                            } else {
                                done as f32 / total as f32
                            };
                            if let Some(task) = this.tasks.iter_mut().find(|task| task.id == id) {
                                task.progress = Some(value.clamp(0.0, 1.0));
                            }
                            cx.notify();
                        })
                        .ok();
                        false
                    }
                    TaskEvent::Done(ok) => {
                        success = ok;
                        true
                    }
                };
                if stop {
                    break;
                }
            }
            weak.update(cx, |this, cx| {
                let cancelled = this
                    .tasks
                    .iter()
                    .find(|task| task.id == id)
                    .map(|task| task.handle.cancelled.load(Ordering::SeqCst))
                    .unwrap_or(false);
                if let Some(task) = this.tasks.iter_mut().find(|task| task.id == id) {
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
                    let text = this
                        .t_args("error.save_instance", &[("error", &error.to_string())])
                        .to_string();
                    this.push_task_log(id, text, cx);
                }
                if refresh {
                    this.reload(cx);
                    let task_instance = this
                        .tasks
                        .iter()
                        .find(|task| task.id == id)
                        .and_then(|task| task.instance.clone());
                    if task_instance.as_deref() == Some(this.cfg_name.as_ref()) {
                        let name = this.cfg_name.clone();
                        this.load_mods(&name, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_task(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(task) = self.tasks.iter().find(|task| task.id == id) else {
            return;
        };
        if task.state != TaskState::Running {
            return;
        }
        task.handle.cancelled.store(true, Ordering::SeqCst);
        if let Some(child) = task.handle.child.lock().unwrap().take() {
            std::thread::spawn(move || kill_tree(child));
        }
        cx.notify();
    }

    fn effective_launch(&self, name: &str, cx: &Context<Self>) -> (String, LaunchParams) {
        let dir = Self::value(&self.dir_input, cx);
        let config = instance::read_launch_config(&dir, name);
        let params = LaunchParams {
            username: config
                .username
                .unwrap_or_else(|| Self::value(&self.username_input, cx)),
            memory: config
                .memory
                .or_else(|| self.memory_value(&self.memory_input, cx)),
            java: config
                .java
                .unwrap_or_else(|| Self::value(&self.java_input, cx)),
            mirror: config
                .mirror
                .unwrap_or_else(|| Self::value(&self.mirror_input, cx)),
            maven: config
                .maven
                .unwrap_or_else(|| Self::value(&self.maven_input, cx)),
            isolate: instance::is_isolated(&dir, name),
        };
        (dir, params)
    }

    fn launch_instance(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(id) = self.running_task_for(name) {
            self.active_task = Some(id);
            cx.notify();
            return;
        }
        let (dir, params) = self.effective_launch(name, cx);
        if dir.is_empty() {
            cx.notify();
            return;
        }
        let mut args = vec![
            String::from("launch"),
            String::from("--dir"),
            dir,
            String::from("--name"),
            name.to_string(),
            String::from("--memory"),
            params.memory.unwrap_or(4096).to_string(),
        ];
        if self.account_offline {
            args.push(String::from("--username"));
            args.push(
                self.account
                    .as_ref()
                    .map(|account| account.to_string())
                    .unwrap_or_default(),
            );
        } else if self.account.is_none() {
            args.push(String::from("--username"));
            args.push(params.username);
        }
        if params.isolate {
            args.push(String::from("--isolate"));
        }
        push_option(&mut args, "--java", &params.java);
        push_option(&mut args, "--mirror", &params.mirror);
        push_option(&mut args, "--maven", &params.maven);

        let title = self.t_args("task.title.launch", &[("name", name)]);
        self.start_task(
            title,
            Some(SharedString::from(name)),
            args,
            false,
            None,
            cx,
        );
    }

    fn login_microsoft(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.running_task_for(ACCOUNT_TASK) {
            self.active_task = Some(id);
            cx.notify();
            return;
        }
        self.start_task(
            self.t("task.title.login"),
            Some(SharedString::from(ACCOUNT_TASK)),
            vec![String::from("login")],
            true,
            None,
            cx,
        );
    }

    fn switch_account(&mut self, name: String, key: String, cx: &mut Context<Self>) {
        if let Some(id) = self.running_task_for(ACCOUNT_TASK) {
            self.active_task = Some(id);
            cx.notify();
            return;
        }
        let title = self.t_args("task.title.account", &[("name", &name)]);
        self.start_task(
            title,
            Some(SharedString::from(ACCOUNT_TASK)),
            vec![String::from("account"), String::from("use"), key],
            true,
            None,
            cx,
        );
    }

    fn remove_account(&mut self, name: String, key: String, cx: &mut Context<Self>) {
        if let Some(id) = self.running_task_for(ACCOUNT_TASK) {
            self.active_task = Some(id);
            cx.notify();
            return;
        }
        let title = self.t_args("task.title.account.remove", &[("name", &name)]);
        self.start_task(
            title,
            Some(SharedString::from(ACCOUNT_TASK)),
            vec![String::from("account"), String::from("remove"), key],
            true,
            None,
            cx,
        );
    }

    fn add_offline_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = Self::value(&self.offline_name_input, cx).trim().to_string();
        if name.is_empty() {
            return;
        }
        if let Some(id) = self.running_task_for(ACCOUNT_TASK) {
            self.active_task = Some(id);
            cx.notify();
            return;
        }
        Self::set_input(&self.offline_name_input, "", window, cx);
        self.offline_form_open = false;
        let title = self.t_args("task.title.account.add", &[("name", &name)]);
        self.start_task(
            title,
            Some(SharedString::from(ACCOUNT_TASK)),
            vec![String::from("account"), String::from("add-offline"), name],
            true,
            None,
            cx,
        );
    }

    fn load_mods(&mut self, name: &str, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let (event_tx, event_rx) = channel::unbounded::<Vec<mods::ModEntry>>();
        let instance = SharedString::from(name.to_string());
        let scan_instance = instance.clone();
        std::thread::spawn(move || {
            let notify_tx = event_tx.clone();
            let _ = mods::list_with_progress(&dir, &scan_instance, move |entries| {
                let _ = notify_tx.send_blocking(entries.to_vec());
            });
        });

        cx.spawn(async move |weak, cx| {
            while let Ok(entries) = event_rx.recv().await {
                weak.update(cx, |this, cx| {
                    if this.cfg_name == instance {
                        this.mods = entries;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_mod(&mut self, file: &str, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let name = self.cfg_name.to_string();
        if dir.is_empty() || name.is_empty() {
            return;
        }
        if let Some(id) = self.running_task_for(&name) {
            self.active_task = Some(id);
            cx.notify();
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
            String::from(file),
        ];
        let title = self.t_args("task.title.mods_toggle", &[("name", &name)]);
        self.start_task(title, Some(SharedString::from(name)), args, true, None, cx);
    }

    fn remove_mod(&mut self, file: &str, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let name = self.cfg_name.to_string();
        if dir.is_empty() || name.is_empty() {
            return;
        }
        if let Some(id) = self.running_task_for(&name) {
            self.active_task = Some(id);
            cx.notify();
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
            String::from(file),
        ];
        let title = self.t_args("task.title.mods_remove", &[("name", &name)]);
        self.start_task(title, Some(SharedString::from(name)), args, true, None, cx);
    }

    fn pick_path(&mut self, target: PathTarget, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(target, PathTarget::ScanJavaDir) {
            let options = PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some(self.t("settings.java.scan_dir")),
            };
            let receiver = cx.prompt_for_paths(options);
            cx.spawn_in(window, async move |weak, cx| {
                let Ok(Ok(Some(paths))) = receiver.await else {
                    return;
                };
                let Some(path) = paths.first() else {
                    return;
                };
                let value = path.to_string_lossy().to_string();
                if let Err(error) = crate::java::add_user_path(&value) {
                    eprintln!("error: {error}");
                    return;
                }
                weak.update_in(cx, |this, window, cx| this.detect_java(window, cx))
                    .ok();
            })
            .detach();
            return;
        }

        let (entity, directories, title) = match target {
            PathTarget::GameDir => (self.dir_input.clone(), true, "settings.game_dir.title"),
            PathTarget::Java => (self.java_input.clone(), true, "settings.java.title"),
            PathTarget::InstanceJava => (self.cfg_java_input.clone(), true, "settings.java.title"),
            PathTarget::InstallJava => (self.install_java_input.clone(), true, "settings.java.title"),
            PathTarget::ImportFile => {
                (
                    self.import_file_input.clone(),
                    false,
                    "download.import.file.title",
                )
            }
            PathTarget::ScanJavaDir => return,
        };
        let options = PathPromptOptions {
            files: !directories,
            directories,
            multiple: false,
            prompt: Some(self.t(title)),
        };
        let receiver = cx.prompt_for_paths(options);
        cx.spawn_in(window, async move |weak, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.first() else {
                return;
            };
            let value = path.to_string_lossy().to_string();
            weak.update_in(cx, |_this, window, cx| {
                Self::set_input(&entity, &value, window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_folder(&mut self, name: &str, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let path = Path::new(&dir).join("versions").join(name);
        if !path.is_dir() {
            cx.notify();
            return;
        }
        let _ = open_directory(&path);
        cx.notify();
    }

    fn back_to_instances(&mut self, cx: &mut Context<Self>) {
        self.instance_page = 0;
        self.cfg_name = SharedString::default();
        cx.notify();
    }

    fn install_version(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        if dir.is_empty() {
            return;
        }
        let version = Self::value(&self.install_version_input, cx);
        if version.is_empty() {
            return;
        }
        let name = Self::value(&self.install_name_input, cx);
        let loader = Self::value(&self.install_loader_input, cx);
        let loader_version = Self::value(&self.install_loader_version_input, cx);
        let username = Self::value(&self.install_username_input, cx);
        let java = Self::value(&self.install_java_input, cx);
        let mirror = Self::value(&self.install_mirror_input, cx);
        let maven = Self::value(&self.install_maven_input, cx);
        let memory = self.memory_value(&self.install_memory_input, cx);

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

        let instance = (!name.is_empty()).then(|| SharedString::from(name.clone()));
        let save = if name.is_empty() {
            None
        } else {
            Some((
                dir,
                name,
                LaunchConfig {
                    username: Some(username),
                    memory,
                    java: Some(java),
                    mirror: Some(mirror),
                    maven: Some(maven),
                    isolate: Some(self.install_isolate),
                },
            ))
        };
        let title = self.t_args("task.title.install", &[("version", &version)]);
        self.start_task(
            title,
            instance,
            args,
            true,
            save,
            cx,
        );
    }

    fn load_game_versions(&mut self, cx: &mut Context<Self>) {
        if matches!(&self.install_versions_state, LoadState::Loading) {
            return;
        }

        let mirror = Self::value(&self.install_mirror_input, cx);
        let maven = Self::value(&self.install_maven_input, cx);
        self.install_versions_state = LoadState::Loading;
        cx.notify();

        let task = cx.background_spawn(async move {
            let origins = crate::download::Origins {
                mojang: (!mirror.is_empty()).then_some(mirror.as_str()),
                maven: (!maven.is_empty()).then_some(maven.as_str()),
            };
            catalog::game_versions(&origins)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(versions) => {
                        this.install_versions = versions;
                        this.install_versions_state = LoadState::Ready;
                    }
                    Err(error) => {
                        this.install_versions_state = LoadState::Failed(SharedString::from(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_loader_versions(&mut self, cx: &mut Context<Self>) {
        let loader = Self::value(&self.install_loader_input, cx);
        let version = Self::value(&self.install_version_input, cx);
        if loader.is_empty() || version.is_empty() {
            self.install_loader_versions.clear();
            self.install_loader_versions_key = None;
            self.install_loader_versions_state = LoadState::Idle;
            cx.notify();
            return;
        }

        let key = (loader.clone(), version.clone());
        if self.install_loader_versions_key.as_ref() == Some(&key)
            && matches!(
                &self.install_loader_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            return;
        }

        let mirror = Self::value(&self.install_mirror_input, cx);
        let maven = Self::value(&self.install_maven_input, cx);
        self.install_loader_versions.clear();
        self.install_loader_versions_key = Some(key.clone());
        self.install_loader_versions_state = LoadState::Loading;
        cx.notify();

        let task = cx.background_spawn(async move {
            let origins = crate::download::Origins {
                mojang: (!mirror.is_empty()).then_some(mirror.as_str()),
                maven: (!maven.is_empty()).then_some(maven.as_str()),
            };
            catalog::loader_versions(&loader, &version, &origins)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.install_loader_versions_key.as_ref() != Some(&key) {
                    return;
                }

                match result {
                    Ok(versions) => {
                        this.install_loader_versions =
                            versions.into_iter().map(SharedString::from).collect();
                        this.install_loader_versions_state = LoadState::Ready;
                    }
                    Err(error) => {
                        this.install_loader_versions_state =
                            LoadState::Failed(SharedString::from(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_version_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.install_version_picker = !self.install_version_picker;
        Self::set_input(&self.install_version_query, "", window, cx);
        if self.install_version_picker
            && !matches!(
                &self.install_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            self.load_game_versions(cx);
        }
        cx.notify();
    }

    fn toggle_download_version_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.download_version_picker = !self.download_version_picker;
        Self::set_input(&self.download_version_query, "", window, cx);
        if self.download_version_picker
            && !matches!(
                &self.install_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            self.load_game_versions(cx);
        }
        cx.notify();
    }

    fn toggle_loader_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.install_loader_picker = !self.install_loader_picker;
        Self::set_input(&self.install_loader_query, "", window, cx);
        if self.install_loader_picker {
            self.load_loader_versions(cx);
        }
        cx.notify();
    }

    fn version_kind(&self, kind: &str) -> SharedString {
        match kind {
            "release" => self.t("version.type.release"),
            "snapshot" => self.t("version.type.snapshot"),
            "old_beta" => self.t("version.type.old_beta"),
            "old_alpha" => self.t("version.type.old_alpha"),
            _ => SharedString::from(kind.to_string()),
        }
    }

    fn import_pack(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        if dir.is_empty() {
            return;
        }
        let file = Self::value(&self.import_file_input, cx);
        if file.is_empty() {
            return;
        }
        let name = Self::value(&self.import_name_input, cx);
        let java = Self::value(&self.java_input, cx);
        let mirror = Self::value(&self.mirror_input, cx);
        let maven = Self::value(&self.maven_input, cx);

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
        if self.import_optional {
            args.push(String::from("--with-optional"));
        }

        let instance = (!name.is_empty()).then(|| SharedString::from(name.clone()));
        self.start_task(
            self.t("task.title.import"),
            instance,
            args,
            true,
            None,
            cx,
        );
    }

    fn import_modrinth(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        if dir.is_empty() {
            return;
        }
        let slug = self
            .mod_detail
            .as_ref()
            .map(|detail| detail.slug.clone())
            .unwrap_or_else(|| Self::value(&self.mod_slug_input, cx));
        if slug.is_empty() {
            return;
        }
        let name = Self::value(&self.import_name_input, cx);
        let java = Self::value(&self.java_input, cx);
        let mirror = Self::value(&self.mirror_input, cx);
        let maven = Self::value(&self.maven_input, cx);

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
        if let Some(version) = self.effective_mod_version(cx) {
            args.push(String::from("--pack-version"));
            args.push(version);
        }
        if self.import_optional {
            args.push(String::from("--with-optional"));
        }

        let instance = (!name.is_empty()).then(|| SharedString::from(name.clone()));
        self.start_task(
            self.t("task.title.import"),
            instance,
            args,
            true,
            None,
            cx,
        );
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

    fn search_mods(&mut self, cx: &mut Context<Self>) {
        let query = Self::value(&self.mod_query_input, cx);
        if query.is_empty() {
            return;
        }
        if matches!(self.search_results_state, LoadState::Loading) {
            return;
        }

        let project_type = String::from(self.search_type());
        let key = project_type.clone();
        let loaders: Vec<String> = if matches!(self.download_tab, 2 | 4) {
            split_values(&Self::value(&self.mod_loader_input, cx))
        } else {
            Vec::new()
        };
        let versions = if project_type == "modpack" {
            split_values(&Self::value(&self.mod_version_input, cx))
        } else {
            let instance = Self::value(&self.mod_instance_input, cx);
            self.instances
                .iter()
                .find(|row| row.name.as_ref() == instance.as_str())
                .map(|row| vec![row.version.to_string()])
                .unwrap_or_default()
        };
        let limit: u32 = Self::value(&self.mod_limit_input, cx).parse().unwrap_or(10);

        self.search_results.clear();
        self.search_icons.clear();
        self.search_results_key = Some(key.clone());
        self.search_results_state = LoadState::Loading;
        cx.notify();

        let task = cx
            .background_spawn(async move { modrinth::search(&query, &project_type, &loaders, &versions, limit) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.search_results_key.as_ref() != Some(&key) {
                    return;
                }

                match result {
                    Ok(hits) => {
                        this.search_results = hits;
                        this.search_results_state = LoadState::Ready;
                        this.load_search_icons(cx);
                    }
                    Err(error) => {
                        this.search_results_state = LoadState::Failed(SharedString::from(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_search_icons(&mut self, cx: &mut Context<Self>) {
        let pending: Vec<(String, String)> = self
            .search_results
            .iter()
            .filter(|hit| !hit.icon_url.is_empty() && !self.search_icons.contains_key(&hit.slug))
            .map(|hit| (hit.slug.clone(), hit.icon_url.clone()))
            .collect();

        for (slug, url) in pending {
            let task = cx.background_spawn(async move {
                crate::download::get_bytes(&url)
                    .ok()
                    .map(|bytes| Arc::new(Image::from_bytes(image_format_from_url(&url), bytes)))
            });
            cx.spawn(async move |this, cx| {
                let image = task.await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(image) = image {
                        this.search_icons.insert(slug, image);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    fn render_search_results(&self, cx: &mut Context<Self>) -> AnyElement {
        if matches!(self.search_results_state, LoadState::Idle) {
            return div().into_any_element();
        }

        let body: AnyElement = match &self.search_results_state {
            LoadState::Idle => div().into_any_element(),
            LoadState::Loading => div()
                .px(px(20.))
                .py(px(14.))
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("download.results.loading"))
                .into_any_element(),
            LoadState::Failed(error) => v_flex()
                .px(px(20.))
                .py(px(14.))
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(skin::c(skin::DANGER_FG))
                        .child(self.t_args("download.results.error", &[("error", error.as_ref())])),
                )
                .child(
                    text_button("search-results-retry", self.t("common.retry")).on_click(
                        cx.listener(|this, _, _, cx| this.search_mods(cx)),
                    ),
                )
                .into_any_element(),
            LoadState::Ready if self.search_results.is_empty() => div()
                .px(px(20.))
                .py(px(14.))
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("download.results.empty"))
                .into_any_element(),
            LoadState::Ready => {
                let selected = Self::value(&self.mod_slug_input, cx);
                let mut list = v_flex()
                    .id("search-results-list")
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .gap(px(1.))
                    .px(px(10.))
                    .py(px(10.));
                for hit in &self.search_results {
                    let slug = hit.slug.clone();
                    let id_slug = slug.clone();
                    let title = hit.title.clone();
                    let description = clip_text(&hit.description, 96);
                    let downloads = self.t_args(
                        "download.results.downloads",
                        &[("count", &hit.downloads.to_string())],
                    );
                    let active = !slug.is_empty() && slug == selected;
                    let icon = search_icon_element(self, hit, &title);
                    let hit_open = hit.clone();
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!("search-result-{id_slug}")))
                            .w_full()
                            .min_w_0()
                            .px(px(10.))
                            .py(px(7.))
                            .rounded(px(6.))
                            .gap(px(12.))
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .when(active, |style| style.bg(skin::c(skin::ACTIVE)))
                            .hover(|style| style.bg(skin::c(skin::HOVER)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                Self::set_input(&this.mod_slug_input, &slug, window, cx);
                                this.download_instance_picker = false;
                                this.open_mod_detail(&hit_open, cx);
                            }))
                            .child(icon)
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(12.5))
                                            .font_medium()
                                            .text_color(skin::c(skin::TEXT))
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(11.))
                                            .text_color(skin::c(skin::FAINT))
                                            .child(description),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(skin::c(skin::FAINT))
                                    .child(downloads),
                            ),
                    );
                }
                list.into_any_element()
            }
        };

        panel()
            .child(row().child(row_title(self.t("download.results.title"))))
            .child(sep())
            .child(body)
            .into_any_element()
    }

    fn toggle_download_instance_picker(&mut self, cx: &mut Context<Self>) {
        self.download_instance_picker = !self.download_instance_picker;
        if self.download_instance_picker {
            let dir = Self::value(&self.dir_input, cx);
            self.instances = list_instances(&dir);
        }
        cx.notify();
    }

    fn render_instance_options(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.instances.is_empty() {
            return div()
                .px(px(20.))
                .py(px(14.))
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("instances.empty"))
                .into_any_element();
        }

        let selected = Self::value(&self.mod_instance_input, cx);
        let mut list = v_flex()
            .id("download-instance-list")
            .max_h(px(240.))
            .overflow_y_scroll()
            .gap(px(1.))
            .px(px(10.))
            .py(px(10.));
        for (index, entry) in self.instances.clone().into_iter().enumerate() {
            let name = entry.name.clone();
            let pick_name = name.clone();
            let meta = if entry.loader.is_empty() {
                entry.version.clone()
            } else {
                SharedString::from(format!("{} · {}", entry.version, entry.loader))
            };
            let active = name == selected;
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("download-instance-{index}")))
                    .w_full()
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(6.))
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .cursor(CursorStyle::PointingHand)
                    .when(active, |style| style.bg(skin::c(skin::ACTIVE)))
                    .hover(|style| style.bg(skin::c(skin::HOVER)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        Self::set_input(&this.mod_instance_input, &pick_name, window, cx);
                        this.download_instance_picker = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.5))
                            .text_color(skin::c(skin::TEXT))
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(skin::c(skin::FAINT))
                            .child(meta),
                    ),
            );
        }
        list.into_any_element()
    }

    fn add_mod(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let name = Self::value(&self.mod_instance_input, cx);
        let Some(detail) = self.mod_detail.clone() else {
            return;
        };
        let slug = detail.slug.clone();
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
            String::from(self.search_type()),
        ];
        if let Some(version) = self.effective_mod_version(cx) {
            args.push(String::from("--version"));
            args.push(version);
        }

        let title = self.t_args("task.title.add", &[("slug", &slug)]);
        self.start_task(
            title,
            Some(SharedString::from(name)),
            args,
            false,
            None,
            cx,
        );
    }

    fn open_mod_detail(&mut self, hit: &modrinth::Hit, cx: &mut Context<Self>) {
        self.mod_detail = Some(hit.clone());
        self.load_mod_versions(&hit.slug, cx);
    }

    fn close_mod_detail(&mut self, cx: &mut Context<Self>) {
        self.mod_detail = None;
        cx.notify();
    }

    fn load_mod_versions(&mut self, slug: &str, cx: &mut Context<Self>) {
        if self.mod_versions_key.as_deref() == Some(slug)
            && matches!(
                self.mod_versions_state,
                LoadState::Loading | LoadState::Ready
            )
        {
            return;
        }
        self.mod_versions.clear();
        self.mod_version_selected = None;
        self.mod_versions_key = Some(slug.to_string());
        self.mod_versions_state = LoadState::Loading;
        cx.notify();

        let key = slug.to_string();
        let fetch = key.clone();
        let task = cx.background_spawn(async move { modrinth::project_versions(&fetch) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.mod_versions_key.as_deref() != Some(key.as_str()) {
                    return;
                }

                match result {
                    Ok(list) => {
                        this.mod_versions = list;
                        this.mod_versions_state = LoadState::Ready;
                    }
                    Err(error) => {
                        this.mod_versions_state = LoadState::Failed(SharedString::from(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn filtered_mod_versions(&self, cx: &Context<Self>) -> Vec<modrinth::ProjectVersion> {
        if self.download_tab == 1 {
            return self.mod_versions.clone();
        }
        let mut loaders = if matches!(self.download_tab, 2 | 4) {
            split_values(&Self::value(&self.mod_loader_input, cx))
        } else {
            Vec::new()
        };
        let instance = Self::value(&self.mod_instance_input, cx);
        let row = self
            .instances
            .iter()
            .find(|row| row.name.as_ref() == instance.as_str());
        let mc_version = row.map(|row| row.version.to_string());
        if loaders.is_empty() && self.download_tab == 2 {
            if let Some(row) = row {
                if row.loader.as_ref() != "vanilla" {
                    loaders = vec![row.loader.to_string()];
                }
            }
        }

        self.mod_versions
            .iter()
            .filter(|version| {
                mc_version
                    .as_ref()
                    .map_or(true, |mc| {
                        version.game_versions.iter().any(|item| item == mc)
                    })
                    && (loaders.is_empty()
                        || version.loaders.iter().any(|item| loaders.contains(item)))
            })
            .cloned()
            .collect()
    }

    fn effective_mod_version(&self, cx: &Context<Self>) -> Option<String> {
        if self.mod_detail.is_none() {
            return None;
        }
        let filtered = self.filtered_mod_versions(cx);
        if let Some(selected) = &self.mod_version_selected {
            if filtered.iter().any(|version| &version.id == selected) {
                return Some(selected.clone());
            }
        }
        if let Some(first) = filtered.first() {
            return Some(first.id.clone());
        }
        self.mod_versions.first().map(|version| version.id.clone())
    }

    fn render_project_detail(
        &self,
        hit: &modrinth::Hit,
        pack: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = search_icon_element(self, hit, &hit.title);
        let downloads = self.t_args(
            "download.results.downloads",
            &[("count", &hit.downloads.to_string())],
        );
        let description = clip_text(&hit.description, 160);
        let selected = self.effective_mod_version(cx);
        let filtered = self.filtered_mod_versions(cx);
        let prefix = if pack { "pack" } else { "mod" };
        let versions_title = if pack {
            self.t("download.pack.versions.title")
        } else {
            self.t("download.mod.versions.title")
        };

        let versions_body: AnyElement = match &self.mod_versions_state {
            LoadState::Idle | LoadState::Loading => div()
                .px(px(20.))
                .py(px(14.))
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("download.mod.versions.loading"))
                .into_any_element(),
            LoadState::Failed(error) => v_flex()
                .px(px(20.))
                .py(px(14.))
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(skin::c(skin::DANGER_FG))
                        .child(self.t_args(
                            "download.mod.versions.error",
                            &[("error", error.as_ref())],
                        )),
                )
                .child(
                    text_button(
                        SharedString::from(format!("{prefix}-versions-retry")),
                        self.t("common.retry"),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let slug = this
                            .mod_detail
                            .as_ref()
                            .map(|detail| detail.slug.clone())
                            .unwrap_or_default();
                        this.load_mod_versions(&slug, cx);
                    })),
                )
                .into_any_element(),
            LoadState::Ready if filtered.is_empty() => div()
                .px(px(20.))
                .py(px(14.))
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("download.mod.versions.empty"))
                .into_any_element(),
            LoadState::Ready => {
                let mut list = v_flex()
                    .id(SharedString::from(format!("{prefix}-version-list")))
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .gap(px(1.))
                    .px(px(10.))
                    .py(px(10.));
                for version in &filtered {
                    let id = version.id.clone();
                    let number = version.number.clone();
                    let name = if version.name.is_empty() || version.name == version.number {
                        String::new()
                    } else {
                        version.name.clone()
                    };
                    let loaders = version.loaders.join(", ");
                    let date = version.date.get(..10).unwrap_or_default().to_string();
                    let active = selected.as_deref() == Some(version.id.as_str());
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!(
                                "{prefix}-version-{}",
                                version.id
                            )))
                            .w_full()
                            .px(px(10.))
                            .py(px(7.))
                            .rounded(px(6.))
                            .gap(px(12.))
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .when(active, |style| style.bg(skin::c(skin::ACTIVE)))
                            .hover(|style| style.bg(skin::c(skin::HOVER)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mod_version_selected = Some(id.clone());
                                cx.notify();
                            }))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(12.5))
                                            .font_medium()
                                            .text_color(skin::c(skin::TEXT))
                                            .child(number),
                                    )
                                    .when(!name.is_empty(), |column| {
                                        column.child(
                                            div()
                                                .min_w_0()
                                                .text_size(px(11.))
                                                .text_color(skin::c(skin::FAINT))
                                                .child(name),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(skin::c(skin::FAINT))
                                    .child(SharedString::from(if loaders.is_empty() {
                                        date
                                    } else {
                                        format!("{loaders} · {date}")
                                    })),
                            ),
                    );
                }
                list.into_any_element()
            }
        };

        let target: AnyElement = if pack {
            panel()
                .child(
                    row()
                        .child(row_title(self.t("download.import.name.title")))
                        .child(self.input_control(&self.import_name_input)),
                )
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.import.optional.title")))
                        .child(
                            Switch::new("pack-detail-optional")
                                .checked(self.import_optional)
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.import_optional = *checked;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element()
        } else {
            panel()
                .child(
                    row()
                        .child(row_title(self.t("download.content.instance.title")))
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap(px(6.))
                                .child(self.input_control(&self.mod_instance_input))
                                .child(
                                    text_button(
                                        "pick-download-instance",
                                        self.t("download.instance.pick"),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_download_instance_picker(cx)
                                    })),
                                ),
                        ),
                )
                .when(self.download_instance_picker, |target| {
                    target.child(sep()).child(self.render_instance_options(cx))
                })
                .into_any_element()
        };

        let action = if pack {
            primary_button("pack-detail-import", self.t("download.import.run"))
                .on_click(cx.listener(|this, _, _, cx| this.import_modrinth(cx)))
        } else {
            primary_button("detail-install", self.t("download.content.add")).on_click(
                cx.listener(|this, _, _, cx| this.add_mod(cx)),
            )
        };

        v_flex()
            .gap(px(20.))
            .child(
                panel()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(icon)
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .text_size(px(14.))
                                                    .font_medium()
                                                    .text_color(skin::c(skin::TEXT))
                                                    .child(hit.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .text_size(px(11.))
                                                    .text_color(skin::c(skin::FAINT))
                                                    .child(downloads),
                                            ),
                                    ),
                            )
                            .child(
                                text_button("project-detail-back", self.t("download.mod.back"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_mod_detail(cx)
                                    })),
                            ),
                    )
                    .child(sep())
                    .child(
                        div()
                            .px(px(20.))
                            .py(px(10.))
                            .text_size(px(12.))
                            .text_color(skin::c(skin::MUTED))
                            .child(description),
                    ),
            )
            .child(
                panel()
                    .child(row().child(row_title(versions_title)))
                    .child(sep())
                    .child(versions_body),
            )
            .child(target)
            .child(h_flex().w_full().justify_end().child(action))
            .into_any_element()
    }

    fn open_instance_settings(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        if dir.is_empty() {
            return;
        }
        let config = instance::read_launch_config(&dir, name);
        self.cfg_name = SharedString::from(name.to_string());

        Self::set_input(&self.cfg_username_input, config.username.as_deref().unwrap_or(""), window, cx);
        let memory = config
            .memory
            .map(|value| value.to_string())
            .unwrap_or_default();
        Self::set_input(&self.cfg_memory_input, &memory, window, cx);
        Self::set_input(&self.cfg_java_input, config.java.as_deref().unwrap_or(""), window, cx);
        Self::set_input(&self.cfg_mirror_input, config.mirror.as_deref().unwrap_or(""), window, cx);
        Self::set_input(&self.cfg_maven_input, config.maven.as_deref().unwrap_or(""), window, cx);
        self.cfg_isolate = instance::is_isolated(&dir, name);
        self.cfg_isolate_local = config.isolate.is_some();
        self.cfg_java_required = crate::version::resolve(&dir, name)
            .ok()
            .map(|resolved| crate::java::required_major_for_version(&resolved.json));
        self.cfg_java_note = None;
        self.cfg_saved = config;

        self.mods = Vec::new();
        self.load_mods(name, cx);

        self.instance_page = 2;
        cx.notify();
    }

    fn cfg_current(&self, cx: &Context<Self>) -> LaunchConfig {
        LaunchConfig {
            username: non_empty(Self::value(&self.cfg_username_input, cx)),
            memory: self.memory_value(&self.cfg_memory_input, cx),
            java: non_empty(Self::value(&self.cfg_java_input, cx)),
            mirror: non_empty(Self::value(&self.cfg_mirror_input, cx)),
            maven: non_empty(Self::value(&self.cfg_maven_input, cx)),
            isolate: if self.cfg_isolate_local {
                Some(self.cfg_isolate)
            } else {
                None
            },
        }
    }

    fn save_instance_settings(&mut self, cx: &mut Context<Self>) {
        let dir = Self::value(&self.dir_input, cx);
        let name = self.cfg_name.to_string();
        if dir.is_empty() || name.is_empty() {
            return;
        }
        let current = self.cfg_current(cx);
        let config = LaunchConfig {
            isolate: Some(self.cfg_isolate),
            ..current
        };
        let _ = instance::write_launch_config(&dir, &name, &config);
        self.cfg_isolate_local = true;
        self.cfg_saved = config;
        self.load_mods(&name, cx);
        self.reload(cx);
    }

    fn detect_java(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.java_runtimes_state = LoadState::Loading;
        self.java_picker = true;
        cx.notify();

        let task = cx.background_spawn(async move { crate::java::detect(true) });
        cx.spawn_in(window, async move |view, cx| {
            let runtimes = task.await;
            view.update_in(cx, |this, _window, cx| {
                this.java_runtimes = runtimes;
                this.java_runtimes_state = LoadState::Ready;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn pick_java_runtime(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(runtime) = self.java_runtimes.get(index).cloned() else {
            return;
        };
        Self::set_input(&self.java_input, &runtime.path, window, cx);
        self.java_picker = false;
        self.save_settings(cx);
    }

    fn render_java_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let body: AnyElement = match &self.java_runtimes_state {
            LoadState::Idle | LoadState::Loading => div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("settings.java.detecting"))
                .into_any_element(),
            LoadState::Failed(error) => div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::DANGER_FG))
                .child(error.clone())
                .into_any_element(),
            LoadState::Ready if self.java_runtimes.is_empty() => div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("settings.java.none"))
                .into_any_element(),
            LoadState::Ready => v_flex()
                .gap(px(4.))
                .children(
                    self.java_runtimes
                        .iter()
                        .enumerate()
                        .map(|(index, runtime)| {
                            h_flex()
                                .id(("java-runtime", index))
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap(px(12.))
                                .px(px(10.))
                                .py(px(7.))
                                .rounded(px(7.))
                                .cursor_pointer()
                                .hover(|style| style.bg(skin::c(skin::ACTIVE)))
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .text_color(skin::c(skin::TEXT))
                                                .child(format!("Java {}", runtime.major)),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(skin::c(skin::FAINT))
                                                .child(runtime.arch.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(11.5))
                                        .text_color(skin::c(skin::MUTED))
                                        .child(runtime.path.clone()),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.pick_java_runtime(index, window, cx);
                                }))
                        }),
                )
                .into_any_element(),
        };

        v_flex()
            .px(px(20.))
            .pb(px(12.))
            .gap(px(6.))
            .child(body)
            .into_any_element()
    }

    fn auto_select_instance_java(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(required) = self.cfg_java_required else {
            return;
        };

        let task = cx.background_spawn(async move {
            let runtimes = crate::java::detect(false);
            crate::java::select(&runtimes, required).map(|runtime| runtime.path.clone())
        });
        cx.spawn_in(window, async move |view, cx| {
            let picked = task.await;
            view.update_in(cx, |this, window, cx| {
                match picked {
                    Some(path) => {
                        Self::set_input(&this.cfg_java_input, &path, window, cx);
                        this.cfg_java_note = None;
                    }
                    None => {
                        let major = required.to_string();
                        this.cfg_java_note = Some(
                            this.t_args("settings.java.auto_failed", &[("major", &major)]),
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        if Self::value(&self.memory_input, cx).is_empty() {
            return;
        }
        let value = self.settings_value(cx);
        if value == self.saved_settings {
            return;
        }
        let path = config_path();
        if write_json(&path, &value, self.lang).is_ok() {
            self.saved_settings = value;
        }
        self.reload(cx);
    }
}

struct LaunchParams {
    username: String,
    memory: Option<u32>,
    java: String,
    mirror: String,
    maven: String,
    isolate: bool,
}

const ACCOUNT_TASK: &str = "microsoft";

fn input(
    window: &mut Window,
    value: &str,
    cx: &mut Context<LauncherUi>,
) -> Entity<input::InputState> {
    cx.new(|cx| {
        let value = value.to_string();
        input::InputState::new(window, cx).default_value(value)
    })
}

fn kill_tree(mut child: std::process::Child) {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let mut command = Command::new("taskkill");
        command
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let _ = command.status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn execute(
    exe: PathBuf,
    args: Vec<String>,
    tx: channel::Sender<TaskEvent>,
    task: Arc<TaskHandle>,
    lang: Lang,
) -> bool {
    let mut command = Command::new(exe);
    command
        .args(&args)
        .env("LYYS_GUI", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let child = command.spawn();
    let Ok(mut child) = child else {
        let _ = tx.send_blocking(TaskEvent::Log(
            i18n::text(lang, "log.start_failed").to_string(),
        ));
        let _ = tx.send_blocking(TaskEvent::Done(false));
        return false;
    };

    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || pump(stdout, tx));
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || pump(stderr, tx));
    }

    {
        let Ok(mut slot) = task.child.lock() else {
            let _ = tx.send_blocking(TaskEvent::Done(false));
            return false;
        };
        *slot = Some(child);
    }

    loop {
        let state = {
            let Ok(mut slot) = task.child.lock() else {
                let _ = tx.send_blocking(TaskEvent::Done(false));
                return false;
            };
            match slot.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(Some(status)) => Some(Ok(status)),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                },
                None => Some(Err(std::io::Error::other("cancelled"))),
            }
        };

        match state {
            None => std::thread::sleep(Duration::from_millis(100)),
            Some(Ok(status)) => {
                let success = status.success();
                let _ = tx.send_blocking(TaskEvent::Log(
                    i18n::text_args(lang, "log.exit", &[("status", &status.to_string())])
                        .to_string(),
                ));
                let _ = tx.send_blocking(TaskEvent::Done(success));
                return success;
            }
            Some(Err(error)) => {
                let message = if task.cancelled.load(Ordering::SeqCst) {
                    i18n::text(lang, "log.cancelled").to_string()
                } else {
                    i18n::text_args(lang, "log.wait_failed", &[("error", &error.to_string())])
                        .to_string()
                };
                let _ = tx.send_blocking(TaskEvent::Log(message));
                let _ = tx.send_blocking(TaskEvent::Done(false));
                return false;
            }
        }
    }
}

fn pump<R: Read + Send + 'static>(reader: R, tx: channel::Sender<TaskEvent>) {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                if line.is_empty() {
                    continue;
                }
                let event = match line.strip_prefix("#progress ") {
                    Some(rest) => rest
                        .split_once('/')
                        .and_then(|(done, total)| {
                            Some(TaskEvent::Progress {
                                done: done.trim().parse().ok()?,
                                total: total.trim().parse().ok()?,
                            })
                        })
                        .unwrap_or_else(|| TaskEvent::Log(line.clone())),
                    None => TaskEvent::Log(line.clone()),
                };
                if tx.send_blocking(event).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn push_option(args: &mut Vec<String>, flag: &str, value: &str) {
    if !value.is_empty() {
        args.push(flag.to_string());
        args.push(value.to_string());
    }
}

fn split_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn clip_text(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let clipped: String = trimmed.chars().take(max).collect();
    format!("{clipped}…")
}

fn search_icon_element(launcher: &LauncherUi, hit: &modrinth::Hit, title: &str) -> AnyElement {
    let initial = SharedString::from(
        title
            .trim()
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| String::from("·")),
    );

    match launcher.search_icons.get(&hit.slug) {
        Some(image) => img(image.clone())
            .w(px(32.))
            .h(px(32.))
            .rounded(px(6.))
            .flex_shrink_0()
            .object_fit(ObjectFit::Cover)
            .with_loading({
                let initial = initial.clone();
                move || icon_placeholder(initial.clone()).into_any_element()
            })
            .with_fallback({
                let initial = initial.clone();
                move || icon_placeholder(initial.clone()).into_any_element()
            })
            .into_any_element(),
        None => icon_placeholder(initial).into_any_element(),
    }
}

fn icon_placeholder(initial: SharedString) -> Div {
    div()
        .w(px(32.))
        .h(px(32.))
        .rounded(px(6.))
        .flex_shrink_0()
        .bg(skin::c(skin::FIELD))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .text_color(skin::c(skin::FAINT))
        .child(initial)
}

fn image_format_from_url(url: &str) -> ImageFormat {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "ico" => ImageFormat::Ico,
        _ => ImageFormat::Png,
    }
}

fn list_instances(mc_dir: &str) -> Vec<InstanceRow> {
    let versions = Path::new(mc_dir).join("versions");
    let Ok(entries) = std::fs::read_dir(&versions) else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !versions.join(&name).join(format!("{name}.json")).is_file() {
            continue;
        }

        let (version, loader) = match crate::version::instance_info(mc_dir, &name) {
            Ok(info) => (
                info.mc_version,
                info.loader.unwrap_or_else(|| String::from("vanilla")),
            ),
            Err(_) => (String::from("unknown"), String::from("unknown")),
        };
        let isolated = instance::is_isolated(mc_dir, &name);
        rows.push(InstanceRow {
            name: SharedString::from(name),
            version: SharedString::from(version),
            loader: SharedString::from(loader),
            isolated,
        });
    }

    rows.sort_by(|left, right| left.name.cmp(&right.name));
    rows
}

struct Settings {
    game_dir: String,
    username: String,
    memory: Option<u32>,
    java: String,
    mirror: String,
    maven: String,
    isolate: bool,
    language: Option<String>,
}

impl Settings {
    fn fallback() -> Self {
        Settings {
            game_dir: String::new(),
            username: String::from("Player"),
            memory: Some(4096),
            java: String::new(),
            mirror: String::new(),
            maven: String::new(),
            isolate: false,
            language: None,
        }
    }
}

fn load_settings() -> Settings {
    let Ok(text) = std::fs::read_to_string(config_path()) else {
        return Settings::fallback();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Settings::fallback();
    };
    let default = Settings::fallback();

    Settings {
        game_dir: value["game_dir"]
            .as_str()
            .unwrap_or(&default.game_dir)
            .to_string(),
        username: value["username"]
            .as_str()
            .unwrap_or(&default.username)
            .to_string(),
        memory: value["memory"]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok()),
        java: value["java"].as_str().unwrap_or("").to_string(),
        mirror: value["mirror"].as_str().unwrap_or("").to_string(),
        maven: value["maven"].as_str().unwrap_or("").to_string(),
        isolate: value["isolate"].as_bool().unwrap_or(false),
        language: value["language"].as_str().map(str::to_string),
    }
}

fn config_path() -> PathBuf {
    crate::loader::base_dir()
        .join("lyys_mc_launcher")
        .join("config.json")
}

fn write_json(path: &Path, value: &Value, lang: Lang) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| {
            i18n::text_args(lang, "error.create_dir", &[("error", &e.to_string())]).to_string()
        })?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| {
        i18n::text_args(lang, "error.write_file", &[("error", &e.to_string())]).to_string()
    })
}

impl Render for LauncherUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(skin::c(skin::WINDOW))
            .text_color(skin::c(skin::TEXT))
            .text_size(px(13.))
            .child(self.render_titlebar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(self.render_main(cx)),
            )
    }
}

fn dot(color: u32, size: f32) -> Div {
    div()
        .w(px(size))
        .h(px(size))
        .rounded_full()
        .flex_shrink_0()
        .bg(skin::c(color))
}

#[cfg(target_os = "windows")]
fn open_directory(path: &Path) -> std::io::Result<std::process::Child> {
    Command::new("explorer").arg(path).spawn()
}

#[cfg(target_os = "macos")]
fn open_directory(path: &Path) -> std::io::Result<std::process::Child> {
    Command::new("open").arg(path).spawn()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_directory(path: &Path) -> std::io::Result<std::process::Child> {
    Command::new("xdg-open").arg(path).spawn()
}

fn panel() -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .bg(skin::c(skin::CARD))
        .overflow_hidden()
}

fn row() -> Div {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .px(px(20.))
        .py(px(14.))
}

fn row_text(title: impl Into<SharedString>, desc: impl Into<SharedString>) -> Div {
    let title = title.into();
    let desc = desc.into();
    v_flex()
        .min_w_0()
        .gap(px(3.))
        .child(
            div()
                .text_size(px(13.5))
                .font_medium()
                .text_color(skin::c(skin::TEXT))
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(desc),
        )
}

fn row_title(title: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .text_size(px(13.5))
        .font_medium()
        .text_color(skin::c(skin::TEXT))
        .child(title.into())
}

fn sep() -> Div {
    div().h(px(1.)).mx(px(20.)).bg(skin::c(skin::DIVIDER))
}

fn page_title(title: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(20.))
        .font_semibold()
        .text_color(skin::c(skin::TEXT))
        .child(title.into())
}

fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .h(px(30.))
        .px(px(12.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .bg(skin::c(skin::ACCENT))
        .text_color(skin::c(skin::ACCENT_FG))
        .text_size(px(12.5))
        .font_medium()
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(skin::c(skin::ACCENT_HOVER)))
        .child(label)
}

fn disabled_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .h(px(30.))
        .px(px(12.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .bg(skin::c(skin::FIELD))
        .text_color(skin::c(skin::FAINT))
        .text_size(px(12.5))
        .font_medium()
        .cursor(CursorStyle::Arrow)
        .child(label)
}

fn text_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .h(px(30.))
        .px(px(10.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .text_size(px(12.5))
        .text_color(skin::c(skin::MUTED))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| {
            style
                .text_color(skin::c(skin::TEXT))
                .bg(skin::c(skin::HOVER))
        })
        .child(label)
}

fn danger_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .h(px(30.))
        .px(px(12.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .bg(skin::c(skin::DANGER))
        .text_color(skin::c(skin::DANGER_FG))
        .text_size(px(12.5))
        .font_medium()
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(skin::c(skin::DANGER_HOVER)))
        .child(label)
}

impl LauncherUi {
    fn render_titlebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self
            .tasks
            .iter()
            .filter(|task| task.state == TaskState::Running)
            .count();
        let mut button = text_button("nav-tasks", self.t("nav.tasks")).on_click(cx.listener(
            |this, _, _, cx| {
                if this.tab == 3 {
                    this.tab = this.prev_tab;
                } else {
                    this.prev_tab = this.tab;
                    this.tab = 3;
                }
                this.instance_page = 0;
                cx.notify();
            },
        ));
        if running > 0 {
            button = button.child(
                div()
                    .ml(px(6.))
                    .px(px(6.))
                    .h(px(16.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .bg(skin::c(skin::BUSY))
                    .text_size(px(10.5))
                    .text_color(skin::c(skin::WINDOW))
                    .child(running.to_string()),
            );
        }
        TitleBar::new()
            .bg(skin::c(skin::WINDOW))
            .border_b_1()
            .border_color(skin::c(skin::TITLEBAR_LINE))
            .child(div().flex_1())
            .child(
                div()
                    .pr(px(8.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(button),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(236.))
            .h_full()
            .flex_shrink_0()
            .bg(skin::c(skin::PANEL))
            .px(px(12.))
            .pt(px(6.))
            .pb(px(14.))
            .child(self.side_nav(
                "nav-instances",
                IconName::LayoutDashboard,
                self.t("nav.instances"),
                self.tab == 0,
                0,
                cx,
            ))
            .child(self.side_nav(
                "nav-download",
                IconName::ArrowDown,
                self.t("nav.download"),
                self.tab == 1,
                1,
                cx,
            ))
            .child(
                h_flex().mt(px(18.)).px(px(10.)).pb(px(6.)).child(
                    div()
                        .text_size(px(13.))
                        .text_color(skin::c(skin::MUTED))
                        .child(self.t("nav.section.system")),
                ),
            )
            .child(self.side_nav(
                "nav-accounts",
                IconName::User,
                self.t("nav.accounts"),
                self.tab == 4,
                4,
                cx,
            ))
            .child(self.side_nav(
                "nav-settings",
                IconName::Settings,
                self.t("nav.settings"),
                self.tab == 2,
                2,
                cx,
            ))
            .child(div().flex_1())
            .child(
                v_flex().px(px(10.)).gap(px(2.)).child(
                    div()
                        .text_size(px(11.5))
                        .text_color(skin::c(skin::FAINT))
                        .child(env!("CARGO_PKG_VERSION")),
                ),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn side_nav(
        &self,
        id: &'static str,
        icon: IconName,
        label: impl Into<SharedString>,
        active: bool,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let foreground = if active {
            skin::c(skin::TEXT)
        } else {
            skin::c(skin::SIDEBAR_TEXT)
        };

        h_flex()
            .id(id)
            .w_full()
            .items_center()
            .gap(px(10.))
            .h(px(36.))
            .px(px(10.))
            .rounded(px(7.))
            .when(active, |this| this.bg(skin::c(skin::ACTIVE)))
            .hover(|style| style.bg(skin::c(skin::HOVER)))
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.tab = index;
                this.instance_page = 0;
                this.download_tab = 0;
                cx.notify();
            }))
            .child(Icon::new(icon).with_size(px(16.)).text_color(foreground))
            .child(
                div()
                    .text_size(px(13.5))
                    .text_color(foreground)
                    .child(label),
            )
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(skin::c(skin::CONTENT))
            .child(
                h_flex().w_full().flex_1().min_h_0().justify_center().child(
                    v_flex()
                        .w_full()
                        .max_w(px(820.))
                        .h_full()
                        .min_h_0()
                        .px(px(40.))
                        .pt(px(28.))
                        .child(self.render_page(cx)),
                ),
            )
    }

    fn render_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.instance_page {
            2 => self.render_instance_settings_page(cx).into_any_element(),
            _ => match self.tab {
                0 => self.render_instances(cx).into_any_element(),
                1 => self.render_download(cx).into_any_element(),
                3 => self.render_tasks(cx).into_any_element(),
                4 => self.render_accounts(cx).into_any_element(),
                _ => self.render_general_settings(cx).into_any_element(),
            },
        }
    }

    fn render_instances(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = panel();

        if self.instances.is_empty() {
            list = list.child(
                h_flex().w_full().justify_center().py(px(40.)).child(
                    div()
                        .text_size(px(13.))
                        .text_color(skin::c(skin::FAINT))
                        .child(self.t("instances.empty")),
                ),
            );
        }

        for (index, row_data) in self.instances.clone().into_iter().enumerate() {
            let launch_name = row_data.name.to_string();
            let launch_click = launch_name.clone();
            let folder_name = row_data.name.clone();
            let settings_name = row_data.name.clone();
            let mut meta = format!("{} · {}", row_data.version, row_data.loader);
            if row_data.isolated {
                meta.push_str(&format!(" · {}", self.t("instances.meta.isolated")));
            }
            if index > 0 {
                list = list.child(sep());
            }
            let running = self.running_task_for(&launch_name);
            list =
                list.child(
                    row().child(row_text(row_data.name.clone(), meta)).child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(4.))
                            .child(if let Some(task_id) = running {
                                danger_button(
                                    SharedString::from(format!("cancel-{launch_name}")),
                                    self.t("instances.action.cancel"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.cancel_task(task_id, cx)
                                }))
                                .into_any_element()
                            } else {
                                primary_button(
                                    SharedString::from(format!("launch-{launch_name}")),
                                    self.t("instances.action.launch"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.launch_instance(&launch_click, cx)
                                }))
                                .into_any_element()
                            })
                            .child(
                                text_button(
                                    SharedString::from(format!("folder-{launch_name}")),
                                    self.t("instances.action.folder"),
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.open_folder(&folder_name, cx),
                                )),
                            )
                            .child(
                                text_button(
                                    SharedString::from(format!("settings-{launch_name}")),
                                    self.t("instances.action.settings"),
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.open_instance_settings(&settings_name, window, cx)
                                    },
                                )),
                            ),
                    ),
                );
        }

        v_flex()
            .size_full()
            .gap(px(20.))
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap(px(16.))
                    .child(page_title(self.t("page.instances.title")))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(6.))
                            .pt(px(4.))
                            .child(text_button("refresh", self.t("common.refresh")).on_click(
                                cx.listener(|this, _, _, cx| this.reload(cx)),
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("instance-list")
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .child(list),
            )
    }

    fn render_download(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().gap(px(16.));
        let content_tab = self.download_tab;
        let show_loader = content_tab == 2 || content_tab == 4;
        if self.download_tab == 0 {
            body = body.child(self.render_install_form(cx));
        } else if self.download_tab == 1 {
            if let Some(detail) = self.mod_detail.clone() {
                body = body.child(self.render_project_detail(&detail, true, cx));
            } else {
            let mut pack_search = panel()
                .child(
                    row()
                        .child(row_title(self.t("download.pack.query.title")))
                        .child(self.input_control(&self.mod_query_input)),
                )
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.version.title")))
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .gap(px(8.))
                                .child(self.input_control(&self.mod_version_input))
                                .child(
                                    text_button(
                                        "download-version-list",
                                        if self.download_version_picker {
                                            self.t("common.close")
                                        } else {
                                            self.t("common.list")
                                        },
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_download_version_picker(window, cx)
                                    })),
                                ),
                        ),
                );
            if self.download_version_picker {
                pack_search = pack_search
                    .child(sep())
                    .child(self.render_version_picker_for(
                        &self.mod_version_input,
                        &self.download_version_query,
                        "download-version-list",
                        false,
                        cx,
                    ));
            }
            let pack_search = pack_search
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.limit.title")))
                        .child(self.input_control(&self.mod_limit_input)),
                );
            body = body
                .child(
                    panel()
                        .child(
                            row()
                                .child(row_title(self.t("download.import.file.title")))
                                .child(self.path_control(
                                    &self.import_file_input,
                                    "browse-import-file",
                                    PathTarget::ImportFile,
                                    cx,
                                )),
                        )
                        .child(sep())
                        .child(
                            row()
                                .child(row_title(self.t("download.import.name.title")))
                                .child(self.input_control(&self.import_name_input)),
                        )
                        .child(sep())
                        .child(
                            row()
                                .child(row_title(self.t("download.import.optional.title")))
                                .child(
                                    Switch::new("import-optional")
                                        .checked(self.import_optional)
                                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                            this.import_optional = *checked;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .child(h_flex().w_full().justify_end().child(
                    primary_button("import-run", self.t("download.import.run")).on_click(
                        cx.listener(|this, _, _, cx| this.import_pack(cx)),
                    ),
                ))
                .child(pack_search)
                .child(h_flex().w_full().justify_end().child(
                    primary_button("pack-search-run", self.t("download.pack.search")).on_click(
                        cx.listener(|this, _, _, cx| this.search_mods(cx)),
                    ),
                ))
                .child(self.render_search_results(cx));
            }
        } else if let Some(detail) = self.mod_detail.clone() {
            body = body.child(self.render_project_detail(&detail, false, cx));
        } else {
            let mut search = panel().child(
                row()
                    .child(row_title(self.t("download.pack.query.title")))
                    .child(self.input_control(&self.mod_query_input)),
            );
            if show_loader {
                let label = if content_tab == 4 {
                    row_title(self.t("download.content.shader_loader.title"))
                } else {
                    row_title(self.t("download.content.loader.title"))
                };
                let control = if content_tab == 2 {
                    let current = Self::value(&self.mod_loader_input, cx);
                    h_flex()
                        .flex_shrink_0()
                        .gap(px(2.))
                        .p(px(3.))
                        .rounded(px(9.))
                        .bg(skin::c(skin::PANEL))
                        .child(Self::loader_option(
                            "mod-loader-fabric",
                            "fabric",
                            current == "fabric",
                            self.mod_loader_input.clone(),
                            false,
                            cx,
                        ))
                        .child(Self::loader_option(
                            "mod-loader-forge",
                            "forge",
                            current == "forge",
                            self.mod_loader_input.clone(),
                            false,
                            cx,
                        ))
                        .child(Self::loader_option(
                            "mod-loader-neoforge",
                            "neoforge",
                            current == "neoforge",
                            self.mod_loader_input.clone(),
                            false,
                            cx,
                        ))
                } else if content_tab == 4 {
                    let current = Self::value(&self.mod_loader_input, cx);
                    h_flex()
                        .flex_shrink_0()
                        .gap(px(2.))
                        .p(px(3.))
                        .rounded(px(9.))
                        .bg(skin::c(skin::PANEL))
                        .child(Self::loader_option(
                            "shader-loader-iris",
                            "iris",
                            current == "iris",
                            self.mod_loader_input.clone(),
                            false,
                            cx,
                        ))
                        .child(Self::loader_option(
                            "shader-loader-optifine",
                            "optifine",
                            current == "optifine",
                            self.mod_loader_input.clone(),
                            false,
                            cx,
                        ))
                } else {
                    div().child(self.input_control(&self.mod_loader_input))
                };
                search = search.child(sep()).child(row().child(label).child(control));
            }
            let instance_name = Self::value(&self.mod_instance_input, cx);
            let detected_version = self
                .instances
                .iter()
                .find(|row| row.name.as_ref() == instance_name.as_str())
                .map(|row| row.version.to_string());
            let version_text: SharedString = match detected_version {
                Some(version) => SharedString::from(version),
                None => self.t("download.version.none"),
            };
            let mut search = search
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.content.instance.title")))
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap(px(6.))
                                .child(self.input_control(&self.mod_instance_input))
                                .child(
                                    text_button(
                                        "pick-download-instance",
                                        self.t("download.instance.pick"),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_download_instance_picker(cx)
                                    })),
                                ),
                        ),
                );
            if self.download_instance_picker {
                search = search.child(sep()).child(self.render_instance_options(cx));
            }
            let search = search
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.version.title")))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(12.5))
                                .text_color(skin::c(skin::MUTED))
                                .child(version_text),
                        ),
                );
            let search = search
                .child(sep())
                .child(
                    row()
                        .child(row_title(self.t("download.limit.title")))
                        .child(self.input_control(&self.mod_limit_input)),
                );
            body = body.child(search).child(h_flex().w_full().justify_end().child(
                primary_button("search-run", self.t("download.content.search")).on_click(
                    cx.listener(|this, _, _, cx| this.search_mods(cx)),
                ),
            ))
                .child(self.render_search_results(cx));
        }

        v_flex()
            .size_full()
            .gap(px(20.))
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap(px(16.))
                    .child(page_title(self.t("page.download.title")))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap(px(2.))
                            .p(px(3.))
                            .rounded(px(9.))
                            .bg(skin::c(skin::PANEL))
                            .child(self.segment(
                                "tab-instance",
                                self.t("download.tab.instance"),
                                self.download_tab == 0,
                                0,
                                cx,
                            ))
                            .child(self.segment(
                                "tab-pack",
                                self.t("download.tab.pack"),
                                self.download_tab == 1,
                                1,
                                cx,
                            ))
                            .child(self.segment(
                                "tab-mods",
                                self.t("download.tab.mods"),
                                self.download_tab == 2,
                                2,
                                cx,
                            ))
                            .child(self.segment(
                                "tab-resourcepack",
                                self.t("download.tab.resourcepack"),
                                self.download_tab == 3,
                                3,
                                cx,
                            ))
                            .child(self.segment(
                                "tab-shader",
                                self.t("download.tab.shader"),
                                self.download_tab == 4,
                                4,
                                cx,
                            ))
                            .child(self.segment(
                                "tab-datapack",
                                self.t("download.tab.datapack"),
                                self.download_tab == 5,
                                5,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("download-scroll")
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .child(body),
            )
    }

    fn segment(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        active: bool,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        div()
            .id(id)
            .h(px(26.))
            .px(px(12.))
            .rounded(px(7.))
            .flex()
            .items_center()
            .text_size(px(12.5))
            .when(active, |this| {
                this.bg(skin::c(skin::ACTIVE))
                    .text_color(skin::c(skin::TEXT))
            })
            .when(!active, |this| this.text_color(skin::c(skin::MUTED)))
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _event, window, cx| {
                if this.download_tab != index {
                    this.search_results.clear();
                    this.search_results_state = LoadState::Idle;
                    this.search_results_key = None;
                    this.mod_detail = None;
                    this.download_version_picker = false;
                    this.download_instance_picker = false;
                }
                this.download_tab = index;
                if index == 2 {
                    let current = Self::value(&this.mod_loader_input, cx);
                    if !matches!(current.as_str(), "fabric" | "forge" | "neoforge") {
                        Self::set_input(&this.mod_loader_input, "fabric", window, cx);
                    }
                }
                if index == 4 {
                    let current = Self::value(&this.mod_loader_input, cx);
                    if current != "iris" && current != "optifine" {
                        Self::set_input(&this.mod_loader_input, "iris", window, cx);
                    }
                }
                cx.notify();
            }))
            .child(label)
    }

    fn loader_option(
        id: &'static str,
        value: &'static str,
        active: bool,
        target: Entity<input::InputState>,
        refresh_install: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(26.))
            .px(px(12.))
            .rounded(px(7.))
            .flex()
            .items_center()
            .text_size(px(12.5))
            .when(active, |this| {
                this.bg(skin::c(skin::ACTIVE))
                    .text_color(skin::c(skin::TEXT))
            })
            .when(!active, |this| this.text_color(skin::c(skin::MUTED)))
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _event, window, cx| {
                Self::set_input(&target, if active { "" } else { value }, window, cx);
                if refresh_install {
                    this.install_loader_versions.clear();
                    this.install_loader_versions_key = None;
                    this.install_loader_versions_state = LoadState::Idle;
                    if this.install_loader_picker {
                        this.load_loader_versions(cx);
                    }
                }
                cx.notify();
            }))
            .child(value)
    }

    fn lang_option(
        &self,
        id: &'static str,
        lang: Lang,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(26.))
            .px(px(12.))
            .rounded(px(7.))
            .flex()
            .items_center()
            .text_size(px(12.5))
            .when(active, |this| {
                this.bg(skin::c(skin::ACTIVE))
                    .text_color(skin::c(skin::TEXT))
            })
            .when(!active, |this| this.text_color(skin::c(skin::MUTED)))
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_language(lang, cx);
            }))
            .child(lang.native_name())
    }

    fn render_accounts(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = crate::microsoft::current_account()
            .map(|account| account.uuid)
            .unwrap_or_default();

        let mut body = v_flex().gap(px(16.));

        if self.account_menu_open {
            body = body.child(
                panel()
                    .child(
                        h_flex()
                            .id("account-add-microsoft")
                            .w_full()
                            .h(px(42.))
                            .px(px(20.))
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .hover(|style| style.bg(skin::c(skin::HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.account_menu_open = false;
                                this.login_microsoft(cx);
                            }))
                            .child(row_title(self.t("accounts.add.microsoft"))),
                    )
                    .child(sep())
                    .child(
                        h_flex()
                            .id("account-add-offline")
                            .w_full()
                            .h(px(42.))
                            .px(px(20.))
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .hover(|style| style.bg(skin::c(skin::HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.account_menu_open = false;
                                this.offline_form_open = true;
                                cx.notify();
                            }))
                            .child(row_title(self.t("accounts.add.offline"))),
                    ),
            );
        }

        if self.offline_form_open {
            body = body.child(
                panel()
                    .child(
                        row()
                            .child(row_title(self.t("accounts.offline.name")))
                            .child(self.input_control(&self.offline_name_input)),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(20.))
                            .pb(px(14.))
                            .child(
                                text_button("offline-cancel", self.t("common.cancel")).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.offline_form_open = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                primary_button("offline-add", self.t("common.add")).on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.add_offline_account(window, cx)
                                    }),
                                ),
                            ),
                    ),
            );
        }

        let mut list = panel();
        if self.accounts.is_empty() {
            list = list.child(
                div()
                    .px(px(20.))
                    .py(px(16.))
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::MUTED))
                    .child(self.t("accounts.empty")),
            );
        } else {
            for (index, account) in self.accounts.iter().enumerate() {
                if index > 0 {
                    list = list.child(sep());
                }

                let name = account.name.clone();
                let key = account.uuid.clone();
                let is_current = account.uuid == current;
                let desc = if account.offline {
                    self.t_args("accounts.uuid.offline", &[("uuid", &account.uuid)])
                } else {
                    self.t_args("accounts.uuid", &[("uuid", &account.uuid)])
                };

                let mut actions = h_flex().gap(px(8.));
                if is_current {
                    actions = actions.child(
                        div()
                            .h(px(24.))
                            .px(px(8.))
                            .rounded(px(6.))
                            .flex()
                            .items_center()
                            .bg(skin::c(skin::FIELD))
                            .text_size(px(11.5))
                            .text_color(skin::c(skin::MUTED))
                            .child(self.t("accounts.current")),
                    );
                } else {
                    let use_name = name.clone();
                    let use_key = key.clone();
                    actions = actions.child(
                        text_button(
                            SharedString::from(format!("account-use-{key}")),
                            self.t("accounts.switch"),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_account(use_name.clone(), use_key.clone(), cx)
                        })),
                    );
                }

                let remove_name = name.clone();
                let remove_key = key.clone();
                actions = actions.child(
                    danger_button(
                        SharedString::from(format!("account-remove-{key}")),
                        self.t("accounts.remove"),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_account(remove_name.clone(), remove_key.clone(), cx)
                    })),
                );

                list = list.child(
                    row()
                        .child(row_text(SharedString::from(name), desc))
                        .child(actions),
                );
            }
        }
        body = body.child(list);

        v_flex()
            .size_full()
            .gap(px(20.))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(page_title(self.t("page.accounts.title")))
                    .child(
                        h_flex()
                            .id("account-add")
                            .h(px(30.))
                            .px(px(12.))
                            .rounded(px(7.))
                            .items_center()
                            .gap(px(6.))
                            .bg(skin::c(skin::ACCENT))
                            .text_color(skin::c(skin::ACCENT_FG))
                            .text_size(px(12.5))
                            .font_medium()
                            .cursor(CursorStyle::PointingHand)
                            .hover(|style| style.bg(skin::c(skin::ACCENT_HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.account_menu_open = !this.account_menu_open;
                                this.offline_form_open = false;
                                cx.notify();
                            }))
                            .child(self.t("accounts.add"))
                            .child(
                                Icon::new(IconName::ArrowDown)
                                    .with_size(px(13.))
                                    .text_color(skin::c(skin::ACCENT_FG)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("accounts-scroll")
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .child(body),
            )
    }

    fn render_general_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap(px(20.))
            .child(page_title(self.t("page.settings.title")))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("settings-scroll")
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .child(
                        v_flex()
                            .gap(px(16.))
                            .child(
                                panel()
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.game_dir.title")))
                                            .child(self.path_control(
                                                &self.dir_input,
                                                "browse-game-dir",
                                                PathTarget::GameDir,
                                                cx,
                                            )),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.username.title")))
                                            .child(self.input_control(&self.username_input)),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.memory.title")))
                                            .child(self.input_control(&self.memory_input)),
                                    ),
                            )
                            .child(
                                panel()
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.java.title")))
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(self.path_control(
                                                        &self.java_input,
                                                        "browse-java",
                                                        PathTarget::Java,
                                                        cx,
                                                    ))
                                                    .child(
                                                        text_button(
                                                            "detect-java",
                                                            self.t("settings.java.detect"),
                                                        )
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.detect_java(window, cx)
                                                            },
                                                        )),
                                                    )
                                                    .child(
                                                        text_button(
                                                            "scan-java-dir",
                                                            self.t("settings.java.scan_dir"),
                                                        )
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.pick_path(
                                                                    PathTarget::ScanJavaDir,
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                    ),
                                            ),
                                    )
                                    .when(self.java_picker, |panel| {
                                        panel
                                            .child(sep())
                                            .child(self.render_java_picker(cx))
                                    })
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.mirror.title")))
                                            .child(self.input_control(&self.mirror_input)),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.maven.title")))
                                            .child(self.input_control(&self.maven_input)),
                                    ),
                            )
                            .child(
                                panel().child(
                                    row()
                                        .child(row_title(self.t("settings.language.title")))
                                        .child(
                                            h_flex()
                                                .flex_shrink_0()
                                                .gap(px(2.))
                                                .p(px(3.))
                                                .rounded(px(9.))
                                                .bg(skin::c(skin::PANEL))
                                                .child(self.lang_option(
                                                    "lang-zh",
                                                    Lang::ZhCn,
                                                    self.lang == Lang::ZhCn,
                                                    cx,
                                                ))
                                                .child(self.lang_option(
                                                    "lang-en",
                                                    Lang::EnUs,
                                                    self.lang == Lang::EnUs,
                                                    cx,
                                                ))
                                                .child(self.lang_option(
                                                    "lang-ja",
                                                    Lang::JaJp,
                                                    self.lang == Lang::JaJp,
                                                    cx,
                                                )),
                                        ),
                                ),
                            )
                            .child(
                                panel().child(
                                    row()
                                        .child(row_title(self.t("settings.isolate.title")))
                                        .child(
                                            Switch::new("settings-isolate")
                                                .checked(self.isolate)
                                                .on_click(cx.listener(
                                                    |this, checked: &bool, _, cx| {
                                                        this.isolate = *checked;
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                ),
                            )
                            .child(h_flex().w_full().justify_end().child(
                                if self.settings_value(cx) == self.saved_settings {
                                    disabled_button("save-settings", self.t("settings.save"))
                                } else {
                                    primary_button("save-settings", self.t("settings.save"))
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.save_settings(cx)),
                                        )
                                },
                            )),
                    ),
            )
    }

    fn render_version_picker_for(
        &self,
        target: &Entity<input::InputState>,
        query_input: &Entity<input::InputState>,
        list_id: &'static str,
        install: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = Self::value(query_input, cx).to_ascii_lowercase();
        let filtered: Vec<&GameVersion> = self
            .install_versions
            .iter()
            .filter(|version| {
                query.is_empty()
                    || version.id.to_ascii_lowercase().contains(&query)
                    || version.kind.to_ascii_lowercase().contains(&query)
            })
            .collect();

        let retry_id = SharedString::from(format!("{list_id}-retry"));
        let close_id = SharedString::from(format!("{list_id}-close"));

        let body: AnyElement = match &self.install_versions_state {
            LoadState::Idle | LoadState::Loading => div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("install.version.loading"))
                .into_any_element(),
            LoadState::Failed(error) => v_flex()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(skin::c(skin::DANGER_FG))
                        .child(self.t_args("install.version.error", &[("error", error.as_ref())])),
                )
                .child(
                    text_button(retry_id, self.t("common.retry")).on_click(cx.listener(
                        |this, _, _, cx| this.load_game_versions(cx),
                    )),
                )
                .into_any_element(),
            LoadState::Ready if filtered.is_empty() => div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("install.version.empty"))
                .into_any_element(),
            LoadState::Ready => {
                let mut list = v_flex()
                    .id(SharedString::from(format!("{list_id}-results")))
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .gap(px(1.));
                for (index, version) in filtered.iter().enumerate() {
                    let id = version.id.clone();
                    let kind = self.version_kind(&version.kind);
                    let target = target.clone();
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!("{list_id}-{index}")))
                            .h(px(30.))
                            .px(px(10.))
                            .rounded(px(6.))
                            .items_center()
                            .justify_between()
                            .cursor(CursorStyle::PointingHand)
                            .hover(|style| style.bg(skin::c(skin::HOVER)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                Self::set_input(&target, &id, window, cx);
                                if install {
                                    this.install_version_picker = false;
                                    Self::set_input(&this.install_version_query, "", window, cx);
                                    this.install_loader_versions.clear();
                                    this.install_loader_versions_key = None;
                                    this.install_loader_versions_state = LoadState::Idle;
                                    if this.install_loader_picker {
                                        this.load_loader_versions(cx);
                                    }
                                } else {
                                    this.download_version_picker = false;
                                    Self::set_input(&this.download_version_query, "", window, cx);
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(skin::c(skin::TEXT))
                                    .child(version.id.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(skin::c(skin::FAINT))
                                    .child(kind),
                            ),
                    );
                }
                list.into_any_element()
            }
        };

        let query_input = query_input.clone();
        v_flex()
            .gap(px(8.))
            .px(px(20.))
            .pb(px(14.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(skin::c(skin::FAINT))
                            .child(self.t("install.version.search")),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(self.input_control(&query_input))
                            .child(
                                text_button(close_id, self.t("common.close")).on_click(
                                    cx.listener(move |this, _, window, cx| {
                                        if install {
                                            this.install_version_picker = false;
                                        } else {
                                            this.download_version_picker = false;
                                        }
                                        Self::set_input(&query_input, "", window, cx);
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    fn render_loader_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let loader = Self::value(&self.install_loader_input, cx);
        let version = Self::value(&self.install_version_input, cx);
        let missing = loader.is_empty() || version.is_empty();
        let current_key = (loader, version);
        let stale = self.install_loader_versions_key.as_ref() != Some(&current_key);
        let query = Self::value(&self.install_loader_query, cx).to_ascii_lowercase();
        let filtered: Vec<&SharedString> = self
            .install_loader_versions
            .iter()
            .filter(|version| query.is_empty() || version.to_ascii_lowercase().contains(&query))
            .collect();

        let body: AnyElement = if missing {
            div()
                .text_size(px(12.5))
                .text_color(skin::c(skin::MUTED))
                .child(self.t("install.loader.missing"))
                .into_any_element()
        } else {
            match &self.install_loader_versions_state {
                LoadState::Idle | LoadState::Loading => div()
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::MUTED))
                    .child(self.t("install.loader.loading"))
                    .into_any_element(),
                LoadState::Failed(error) => v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(skin::c(skin::DANGER_FG))
                            .child(
                                self.t_args("install.loader.error", &[("error", error.as_ref())]),
                            ),
                    )
                    .child(
                        text_button("install-loader-retry", self.t("common.retry"))
                            .on_click(cx.listener(|this, _, _, cx| this.load_loader_versions(cx))),
                    )
                    .into_any_element(),
                LoadState::Ready if stale => v_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(skin::c(skin::MUTED))
                            .child(self.t("install.loader.changed")),
                    )
                    .child(
                        text_button("install-loader-refresh", self.t("common.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.load_loader_versions(cx))),
                    )
                    .into_any_element(),
                LoadState::Ready if filtered.is_empty() => div()
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::MUTED))
                    .child(self.t("install.loader.empty"))
                    .into_any_element(),
                LoadState::Ready => {
                    let mut list = v_flex()
                        .id("install-loader-list")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .gap(px(1.));

                    if query.is_empty() {
                        list = list.child(
                            h_flex()
                                .id("install-loader-latest")
                                .h(px(30.))
                                .px(px(10.))
                                .rounded(px(6.))
                                .items_center()
                                .cursor(CursorStyle::PointingHand)
                                .hover(|style| style.bg(skin::c(skin::HOVER)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    Self::set_input(
                                        &this.install_loader_version_input,
                                        "",
                                        window,
                                        cx,
                                    );
                                    this.install_loader_picker = false;
                                    Self::set_input(&this.install_loader_query, "", window, cx);
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(skin::c(skin::MUTED))
                                        .child(self.t("install.loader.latest")),
                                ),
                        );
                    }

                    for (index, version) in filtered.iter().enumerate() {
                        let value = version.to_string();
                        let picked = value.clone();
                        list = list.child(
                            h_flex()
                                .id(SharedString::from(format!("install-loader-{index}")))
                                .h(px(30.))
                                .px(px(10.))
                                .rounded(px(6.))
                                .items_center()
                                .cursor(CursorStyle::PointingHand)
                                .hover(|style| style.bg(skin::c(skin::HOVER)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    Self::set_input(
                                        &this.install_loader_version_input,
                                        &picked,
                                        window,
                                        cx,
                                    );
                                    this.install_loader_picker = false;
                                    Self::set_input(&this.install_loader_query, "", window, cx);
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(skin::c(skin::TEXT))
                                        .child(value),
                                ),
                        );
                    }
                    list.into_any_element()
                }
            }
        };

        v_flex()
            .gap(px(8.))
            .px(px(20.))
            .pb(px(14.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(skin::c(skin::FAINT))
                            .child(self.t("install.loader.search")),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(self.input_control(&self.install_loader_query))
                            .child(
                                text_button("install-loader-close", self.t("common.close"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.install_loader_picker = false;
                                        Self::set_input(&this.install_loader_query, "", window, cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    fn render_install_form(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let loader = Self::value(&self.install_loader_input, cx);
        let mut version_panel = panel().child(
            row()
                .child(row_title(self.t("install.version.title")))
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .gap(px(8.))
                        .child(self.input_control(&self.install_version_input))
                        .child(
                            text_button(
                                "install-version-list",
                                if self.install_version_picker {
                                    self.t("common.close")
                                } else {
                                    self.t("common.list")
                                },
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.toggle_version_picker(window, cx),
                            )),
                        ),
                ),
        );

        if self.install_version_picker {
            version_panel = version_panel
                .child(sep())
                .child(self.render_version_picker_for(
                    &self.install_version_input,
                    &self.install_version_query,
                    "install-version-list",
                    true,
                    cx,
                ));
        }

        version_panel = version_panel
            .child(sep())
            .child(
                row()
                    .child(row_title(self.t("install.name.title")))
                    .child(self.input_control(&self.install_name_input)),
            )
            .child(sep())
            .child(
                row()
                    .child(row_title(self.t("install.loader.title")))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap(px(2.))
                            .p(px(3.))
                            .rounded(px(9.))
                            .bg(skin::c(skin::PANEL))
                            .child(Self::loader_option(
                                "install-loader-fabric",
                                "fabric",
                                loader == "fabric",
                                self.install_loader_input.clone(),
                                true,
                                cx,
                            ))
                            .child(Self::loader_option(
                                "install-loader-forge",
                                "forge",
                                loader == "forge",
                                self.install_loader_input.clone(),
                                true,
                                cx,
                            ))
                            .child(Self::loader_option(
                                "install-loader-neoforge",
                                "neoforge",
                                loader == "neoforge",
                                self.install_loader_input.clone(),
                                true,
                                cx,
                            )),
                    ),
            );

        if self.install_loader_picker {
            version_panel = version_panel
                .child(sep())
                .child(self.render_loader_picker(cx));
        }

        version_panel = version_panel.child(sep()).child(
            row()
                .child(row_title(self.t("install.loader_version.title")))
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .gap(px(8.))
                        .child(self.input_control(&self.install_loader_version_input))
                        .child(
                            text_button(
                                "install-loader-version-list",
                                if self.install_loader_picker {
                                    self.t("common.close")
                                } else {
                                    self.t("common.list")
                                },
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.toggle_loader_picker(window, cx),
                            )),
                        ),
                ),
        );

        v_flex()
            .gap(px(16.))
            .child(version_panel)
            .child(
                panel()
                    .child(
                        row()
                            .child(row_title(self.t("settings.username.title")))
                            .child(self.input_control(&self.install_username_input)),
                    )
                    .child(sep())
                    .child(
                        row()
                            .child(row_title(self.t("settings.memory.title")))
                            .child(self.input_control(&self.install_memory_input)),
                    )
                    .child(sep())
                    .child(
                        row()
                            .child(row_title(self.t("settings.java.title")))
                            .child(self.path_control(
                                &self.install_java_input,
                                "browse-install-java",
                                PathTarget::InstallJava,
                                cx,
                            )),
                    )
                    .child(sep())
                    .child(
                        row()
                            .child(row_title(self.t("settings.mirror.title")))
                            .child(self.input_control(&self.install_mirror_input)),
                    )
                    .child(sep())
                    .child(
                        row()
                            .child(row_title(self.t("settings.maven.title")))
                            .child(self.input_control(&self.install_maven_input)),
                    ),
            )
            .child(
                panel().child(
                    row()
                        .child(row_title(self.t("install.isolate.title")))
                        .child(
                            Switch::new("install-isolate")
                                .checked(self.install_isolate)
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.install_isolate = *checked;
                                    cx.notify();
                                })),
                        ),
                ),
            )
            .child(h_flex().w_full().justify_end().child(
                primary_button("install-run", self.t("install.run")).on_click(
                    cx.listener(|this, _, _, cx| this.install_version(cx)),
                ),
            ))
    }

    fn render_mods_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex().w_full();
        for (index, entry) in self.mods.iter().enumerate() {
            let file = entry.file.clone();
            let toggle_file = file.clone();
            let label = if entry.version.is_empty() {
                entry.id.clone()
            } else {
                format!("{} {}", entry.id, entry.version)
            };
            let source = entry
                .source
                .clone()
                .unwrap_or_else(|| self.t("instance_settings.mods.unknown").to_string());

            if index > 0 {
                list = list.child(sep());
            }
            list = list.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(12.))
                    .px(px(20.))
                    .py(px(10.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.5))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(skin::c(skin::FAINT))
                                    .child(source),
                            ),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("mod-switch-{index}")))
                            .checked(!entry.disabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_mod(&toggle_file, cx);
                            })),
                    )
                    .child(
                        danger_button(
                            SharedString::from(format!("mod-remove-{index}")),
                            self.t("instance_settings.mods.remove"),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_mod(&file, cx);
                        })),
                    ),
            );
        }

        if self.mods.is_empty() {
            list = list.child(
                div()
                    .px(px(20.))
                    .py(px(10.))
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::FAINT))
                    .child(self.t("instance_settings.mods.empty")),
            );
        }

        panel()
            .child(
                row()
                    .child(row_title(self.t("instance_settings.mods.title")))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap(px(8.))
                            .child(
                                text_button("mods-refresh", self.t("common.refresh")).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        let name = this.cfg_name.clone();
                                        this.load_mods(&name, cx);
                                    }),
                                ),
                            ),
                    ),
            )
            .child(sep())
            .child(list)
    }

    fn render_instance_settings_page(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let global_username = Self::value(&self.username_input, cx);
        let global_memory = self
            .memory_value(&self.memory_input, cx)
            .map(|value| value.to_string())
            .unwrap_or_default();
        let global_java = Self::value(&self.java_input, cx);
        let global_mirror = Self::value(&self.mirror_input, cx);
        let global_maven = Self::value(&self.maven_input, cx);
        let java_required_note = self.cfg_java_required.map(|required| {
            self.t_args(
                "settings.java.required",
                &[("major", &required.to_string())],
            )
        });
        v_flex()
            .size_full()
            .gap(px(20.))
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(10.))
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .pt(px(4.))
                            .child(text_button("cfg-back", "←").on_click(
                                cx.listener(|this, _, _, cx| this.back_to_instances(cx)),
                            )),
                    )
                    .child(page_title(self.t("instance_settings.title"))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .id("cfg-scroll")
                    .overflow_y_scroll()
                    .pb(px(6.))
                    .child(
                        v_flex()
                            .gap(px(16.))
                            .child(
                                panel()
                                    .child(
                                        row()
                                            .child(row_text(
                                                self.t("instance_settings.instance"),
                                                self.cfg_name.clone(),
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .text_color(skin::c(skin::MUTED))
                                                    .child(self.t("instance_settings.override")),
                                            ),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.username.title")))
                                            .child(self.cfg_override_control(
                                                &self.cfg_username_input,
                                                "cfg-reset-username",
                                                global_username,
                                                cx,
                                            )),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.memory.title")))
                                            .child(self.cfg_override_control(
                                                &self.cfg_memory_input,
                                                "cfg-reset-memory",
                                                global_memory,
                                                cx,
                                            )),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(
                                                v_flex()
                                                    .gap(px(2.))
                                                    .child(row_title(self.t("settings.java.title")))
                                                    .when(
                                                        self.cfg_java_note.is_some(),
                                                        |column| {
                                                            column.child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(skin::c(
                                                                        skin::DANGER_FG,
                                                                    ))
                                                                    .child(
                                                                        self.cfg_java_note
                                                                            .clone()
                                                                            .unwrap_or_default(),
                                                                    ),
                                                            )
                                                        },
                                                    )
                                                    .when(
                                                        java_required_note.is_some(),
                                                        |column| {
                                                            column.child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(skin::c(skin::FAINT))
                                                                    .child(
                                                                        java_required_note
                                                                            .clone()
                                                                            .unwrap_or_default(),
                                                                    ),
                                                            )
                                                        },
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(self.path_control(
                                                        &self.cfg_java_input,
                                                        "browse-cfg-java",
                                                        PathTarget::InstanceJava,
                                                        cx,
                                                    ))
                                                    .child(self.cfg_source_badge(
                                                        &self.cfg_java_input,
                                                        "cfg-reset-java",
                                                        global_java,
                                                        cx,
                                                    ))
                                                    .child(
                                                        text_button(
                                                            "cfg-auto-java",
                                                            self.t("settings.java.auto"),
                                                        )
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.auto_select_instance_java(
                                                                    window, cx,
                                                                )
                                                            },
                                                        )),
                                                    ),
                                            ),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.mirror.title")))
                                            .child(self.cfg_override_control(
                                                &self.cfg_mirror_input,
                                                "cfg-reset-mirror",
                                                global_mirror,
                                                cx,
                                            )),
                                    )
                                    .child(sep())
                                    .child(
                                        row()
                                            .child(row_title(self.t("settings.maven.title")))
                                            .child(self.cfg_override_control(
                                                &self.cfg_maven_input,
                                                "cfg-reset-maven",
                                                global_maven,
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                panel().child(
                                    row()
                                        .child(row_title(self.t("instance_settings.isolate.title")))
                                        .child(
                                            h_flex()
                                                .flex_shrink_0()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(
                                                    Switch::new("cfg-isolate")
                                                        .checked(self.cfg_isolate)
                                                        .on_click(cx.listener(
                                                            |this, checked: &bool, _, cx| {
                                                                this.cfg_isolate = *checked;
                                                                this.cfg_isolate_local = true;
                                                                cx.notify();
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .text_color(skin::c(if self.cfg_isolate_local {
                                                            skin::TEXT
                                                        } else {
                                                            skin::FAINT
                                                        }))
                                                        .child(if self.cfg_isolate_local {
                                                            self.t("instance_settings.source.local")
                                                        } else {
                                                            self.t("instance_settings.source.unset")
                                                        }),
                                                ),
                                        ),
                                ),
                            )
                            .child(self.render_mods_panel(cx)),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .flex_shrink_0()
                    .justify_end()
                    .child(if self.cfg_current(cx) == self.cfg_saved {
                        disabled_button("cfg-save", self.t("instance_settings.save"))
                    } else {
                        primary_button("cfg-save", self.t("instance_settings.save")).on_click(
                            cx.listener(|this, _, _, cx| this.save_instance_settings(cx)),
                        )
                    }),
            )
    }

    fn path_control(
        &self,
        state: &Entity<input::InputState>,
        id: &'static str,
        target: PathTarget,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .flex_shrink_0()
            .gap(px(8.))
            .child(self.input_control(state))
            .child(
                text_button(id, self.t("common.browse")).on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.pick_path(target, window, cx);
                    }),
                ),
            )
    }

    fn cfg_override_control(
        &self,
        state: &Entity<input::InputState>,
        reset_id: &'static str,
        global: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let input = self.input_control(state);
        h_flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(8.))
            .child(input)
            .child(self.cfg_source_badge(state, reset_id, global, cx))
    }

    fn cfg_source_badge(
        &self,
        state: &Entity<input::InputState>,
        reset_id: &'static str,
        global: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let local = !Self::value(state, cx).is_empty();
        let label = if local {
            self.t("instance_settings.source.local")
        } else {
            self.t("instance_settings.source.global")
        };
        let fallback: SharedString = if global.is_empty() {
            self.t("instance_settings.source.unset")
        } else {
            global.into()
        };
        let reset_label = self.t("instance_settings.reset");
        let state = state.clone();
        h_flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(skin::c(if local { skin::TEXT } else { skin::FAINT }))
                    .child(label),
            )
            .when(!local, |this| {
                this.child(
                    div()
                        .max_w(px(160.))
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(skin::c(skin::FAINT))
                        .child(fallback),
                )
            })
            .when(local, |this| {
                this.child(text_button(reset_id, reset_label).on_click(cx.listener(
                    move |_, _, window, cx| {
                        Self::set_input(&state, "", window, cx);
                        cx.notify();
                    },
                )))
            })
    }

    fn input_control(&self, state: &Entity<input::InputState>) -> impl IntoElement {
        input::Input::new(state)
            .w(px(280.))
            .flex_shrink_0()
            .small()
            .bg(skin::c(skin::FIELD))
            .border_1()
            .border_color(skin::c(skin::FIELD_BORDER))
            .rounded(px(8.))
            .text_size(px(12.5))
    }

    fn render_tasks(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mono = cx.theme().mono_font_family.clone();
        let active = self.active_task;
        let mut list = v_flex()
            .id("task-list")
            .w(px(280.))
            .h_full()
            .flex_shrink_0()
            .gap(px(2.))
            .overflow_y_scroll()
            .pb(px(6.));

        if self.tasks.is_empty() {
            list = list.child(
                div()
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::FAINT))
                    .child(self.t("tasks.empty")),
            );
        }

        for task in self.tasks.iter().rev() {
            let id = task.id;
            let color = match task.state {
                TaskState::Running => skin::BUSY,
                TaskState::Success => skin::OK,
                TaskState::Failed => skin::DANGER_HOVER,
                TaskState::Cancelled => skin::FAINT,
            };
            let state_label = self.t(task_state_key(task.state));
            let selected = active == Some(id);
            let percent = task
                .progress
                .map(|value| format!("{:.0}%", value * 100.0));
            let mut entry = h_flex()
                .id(SharedString::from(format!("task-{id}")))
                .w_full()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .cursor(CursorStyle::PointingHand)
                .when(selected, |this| this.bg(skin::c(skin::ACTIVE)))
                .hover(|style| style.bg(skin::c(skin::HOVER)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_task = Some(id);
                    this.task_log_follow = true;
                    cx.notify();
                }))
                .child(dot(color, 6.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(12.5))
                                .text_color(skin::c(skin::TEXT))
                                .child(task.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(skin::c(skin::FAINT))
                                .child(state_label),
                        ),
                );
            if let Some(percent) = percent {
                entry = entry.child(
                    div()
                        .text_size(px(11.))
                        .text_color(skin::c(skin::MUTED))
                        .child(percent),
                );
            }
            list = list.child(entry);
        }

        let selected = active
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
            .map(|task| {
                let skip = task.log_lines.len().saturating_sub(400);
                (
                    task.id,
                    task.title.clone(),
                    task.state,
                    task.progress,
                    task.log_lines.iter().skip(skip).cloned().collect::<Vec<_>>(),
                )
            });

        let mut detail = v_flex().flex_1().min_w_0().h_full().gap(px(14.));
        if let Some((id, title, state, progress, lines)) = selected {
            let state_label = self.t(task_state_key(state));
            let mut header = h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    v_flex()
                        .min_w_0()
                        .gap(px(4.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(16.))
                                .text_color(skin::c(skin::TEXT))
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(skin::c(skin::FAINT))
                                .child(self.t_args(
                                    "tasks.detail.header",
                                    &[("id", &id.to_string()), ("state", state_label.as_ref())],
                                )),
                        ),
                );
            if state == TaskState::Running {
                header = header.child(
                    danger_button("task-cancel", self.t("tasks.cancel"))
                        .on_click(cx.listener(move |this, _, _, cx| this.cancel_task(id, cx))),
                );
            }
            detail = detail.child(header);

            if let Some(value) = progress {
                detail = detail.child(
                    h_flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .w(px(180.))
                                .h(px(6.))
                                .rounded_full()
                                .bg(skin::c(skin::DIVIDER))
                                .child(
                                    div()
                                        .h_full()
                                        .w(gpui::relative(value))
                                        .rounded_full()
                                        .bg(skin::c(skin::BUSY)),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(skin::c(skin::MUTED))
                                .child(format!("{:.0}%", value * 100.0)),
                        ),
                );
            }

            if self.task_log_follow {
                self.task_log_follow = false;
                self.task_log_scroll.scroll_to_bottom();
            }

            detail = detail.child(
                div()
                    .id("task-log")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.task_log_scroll)
                    .rounded(px(10.))
                    .bg(skin::c(skin::CARD))
                    .px(px(16.))
                    .py(px(12.))
                    .font_family(mono)
                    .text_size(px(11.5))
                    .text_color(skin::c(skin::MUTED))
                    .child(v_flex().gap(px(3.)).children(if lines.is_empty() {
                        vec![self.t("tasks.log.empty")]
                    } else {
                        lines
                    })),
            );
        } else {
            detail = detail.child(
                div()
                    .text_size(px(12.5))
                    .text_color(skin::c(skin::FAINT))
                    .child(self.t("tasks.detail.empty")),
            );
        }

        v_flex()
            .size_full()
            .gap(px(20.))
            .child(page_title(self.t("page.tasks.title")))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(20.))
                    .child(list)
                    .child(detail),
            )
    }
}

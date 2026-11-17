mod account;
mod catalog;
mod cli;
mod download;
mod gui;
mod i18n;
mod installer;
mod instance;
mod java;
mod launch;
mod loader;
mod metadata;
mod microsoft;
mod modpack;
mod modrinth;
mod mods;
mod version;

use account::offline_uuid;
use cli::{AddCli, DownloadCli, ImportCli, LaunchCli, SearchCli};
use launch::{LaunchOptions, build_args};

fn absolute_path(path: &str) -> String {
    std::path::absolute(path)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string())
}

fn main() {
    if std::env::args().len() > 1 {
        match cli::parse() {
            cli::Command::Launch(c) => run_launch(&c),
            cli::Command::Download(c) => run_download(&c),
            cli::Command::Search(c) => run_search(&c),
            cli::Command::Add(c) => run_add(&c),
            cli::Command::Import(c) => run_import(&c),
            cli::Command::Login => run_login(),
            cli::Command::Logout => run_logout(),
            cli::Command::Account(c) => run_account(&c),
            cli::Command::Mods(c) => run_mods(&c),
            cli::Command::Java(c) => run_java(&c),
        }
        return;
    }

    #[cfg(windows)]
    hide_console();

    if let Err(error) = gui::run() {
        eprintln!("gui error: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn hide_console() {
    unsafe extern "system" {
        fn FreeConsole() -> i32;
    }

    unsafe {
        FreeConsole();
    }
}

fn run_login() {
    match microsoft::login() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("login failed: {e}");
            std::process::exit(1);
        }
    }
}

fn run_logout() {
    match microsoft::logout() {
        Ok(()) => println!("logged out"),
        Err(e) => {
            eprintln!("logout failed: {e}");
            std::process::exit(1);
        }
    }
}

fn run_account(c: &cli::AccountCli) {
    let result = match &c.action {
        cli::AccountAction::Use(key) => microsoft::set_current(key).map(|_| {
            let name = microsoft::account_name().unwrap_or_else(|| key.clone());
            println!("current account: {name}");
        }),
        cli::AccountAction::Remove(key) => microsoft::remove_account(key)
            .map(|_| println!("removed account: {key}")),
        cli::AccountAction::AddOffline(name) => microsoft::add_offline(name)
            .map(|_| println!("added offline account: {}", name.trim())),
    };

    if let Err(e) = result {
        eprintln!("account failed: {e}");
        std::process::exit(1);
    }
}

fn run_java(c: &cli::JavaCli) {
    match &c.action {
        cli::JavaAction::List { refresh } => {
            let runtimes = java::detect(*refresh);
            if runtimes.is_empty() {
                println!("no Java runtime found");
                return;
            }
            for runtime in runtimes {
                println!("{}\t{}\t{}", runtime.major, runtime.arch, runtime.path);
            }
        }
        cli::JavaAction::Path { version } => {
            let required = java::required_major(version);
            let runtimes = java::detect(false);
            let Some(runtime) = java::select(&runtimes, required) else {
                eprintln!(
                    "no Java {required}+ found; run `lyys_mc_launcher java list --refresh` to detect installed runtimes"
                );
                std::process::exit(1);
            };
            println!("{}", runtime.path);
        }
        cli::JavaAction::Add { path } => match java::add_user_path(path) {
            Ok(()) => println!("added {path}"),
            Err(error) => {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        },
        cli::JavaAction::Remove { path } => match java::remove_user_path(path) {
            Ok(()) => println!("removed {path}"),
            Err(error) => {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        },
    }
}

fn run_mods(c: &cli::ModCli) {
    let mc_dir = absolute_path(&c.mc_dir);
    let result = match c.action {
        cli::ModsAction::List => mods::list(&mc_dir, &c.instance).map(print_mods),
        cli::ModsAction::Remove => mods::remove(&mc_dir, &c.instance, &c.mods)
            .map(|count| println!("removed {count} mod(s)")),
        cli::ModsAction::Toggle => c.mods.iter().try_for_each(|name| {
            mods::toggle(&mc_dir, &c.instance, name).map(|_| ())
        }),
    };

    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn print_mods(entries: Vec<mods::ModEntry>) {
    println!("{} mod(s):", entries.len());
    for entry in entries {
        let state = if entry.disabled {
            "disabled"
        } else {
            "enabled"
        };
        let source = entry.source.as_deref().unwrap_or("unknown source");
        if entry.version.is_empty() {
            println!("{state} {} {} ({source})", entry.file, entry.id);
        } else {
            println!(
                "{state} {} {} {} ({source})",
                entry.file, entry.id, entry.version
            );
        }
    }
}

fn run_search(c: &SearchCli) {
    match modrinth::search(&c.query, &c.project_type, &c.loaders, &c.versions, c.limit) {
        Ok(hits) if hits.is_empty() => println!("no results"),
        Ok(hits) => {
            for hit in hits {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    hit.slug, hit.project_type, hit.downloads, hit.title, hit.description
                );
            }
        }
        Err(e) => {
            eprintln!("search failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_add(c: &AddCli) {
    let mc_dir = absolute_path(&c.mc_dir);
    let info = match version::instance_info(&mc_dir, &c.instance) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let loaders: Vec<String> = info.loader.into_iter().collect();
    let versions = vec![info.mc_version];

    match modrinth::install(
        &mc_dir,
        &c.instance,
        &c.slug,
        &c.project_type,
        &loaders,
        &versions,
        c.version.as_deref(),
        c.with_optional,
    ) {
        Ok(report) => {
            println!("installed {} file(s):", report.files.len());
            for file in &report.files {
                println!("{}", file);
            }

            if !report.optional.is_empty() {
                println!();
                println!("optional dependencies:");
                for dep in &report.optional {
                    let state = if dep.installed {
                        "installed"
                    } else {
                        "skipped, use --with-optional"
                    };
                    println!("  {}  ({})", dep.name, state);
                }
            }
        }
        Err(e) => {
            eprintln!("failed to add mod: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_download(c: &DownloadCli) {
    let mc_dir = absolute_path(&c.mc_dir);
    let origins = download::Origins {
        mojang: c.mirror.as_deref(),
        maven: c.maven.as_deref(),
    };

    let loader_name = c.loader.as_deref().map(str::to_ascii_lowercase);
    let required = java::required_major(&c.version);

    let result = match loader_name.as_deref() {
        Some("fabric") => loader::install_fabric(
            &mc_dir,
            &c.version,
            c.loader_version.as_deref(),
            c.name.as_deref(),
            &origins,
        ),
        Some("forge") => {
            let java = launch::resolve_java(c.java.as_deref(), required);
            loader::install_forge(
                &mc_dir,
                &c.version,
                c.loader_version.as_deref(),
                c.name.as_deref(),
                &java,
                &origins,
            )
        }
        Some("neoforge") => {
            let java = launch::resolve_java(c.java.as_deref(), required);
            loader::install_neoforge(
                &mc_dir,
                &c.version,
                c.loader_version.as_deref(),
                c.name.as_deref(),
                &java,
                &origins,
            )
        }
        Some(other) => Err(format!(
            "unsupported loader: {other} (supported: fabric, forge, neoforge)"
        )),
        None => {
            let instance = c.name.clone().unwrap_or_else(|| c.version.clone());
            if let Err(e) = loader::validate_instance_id(&instance) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
            match download::download_instance(&mc_dir, &instance, &c.version, &origins) {
                Ok(()) => Ok(instance),
                Err(e) => Err(e),
            }
        }
    };

    match result {
        Ok(id) => println!("instance {} installed", id),
        Err(e) => {
            eprintln!("failed to install: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_import(c: &ImportCli) {
    let mc_dir = absolute_path(&c.mc_dir);
    let cache = std::path::Path::new(&mc_dir).join(".cache");
    if let Err(e) = std::fs::create_dir_all(&cache) {
        eprintln!("failed to create {}: {e}", cache.display());
        std::process::exit(1);
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let plain = download::Origins {
        mojang: None,
        maven: None,
    };

    let (path, source) = if let Some(file) = &c.file {
        (std::path::PathBuf::from(file), file.clone())
    } else if let Some(url) = &c.url {
        let mut name = url
            .split(['?', '#'])
            .next()
            .unwrap_or("modpack")
            .rsplit('/')
            .next()
            .unwrap_or("modpack");
        if !name.ends_with(".zip") && !name.ends_with(".mrpack") {
            name = "modpack.zip";
        }
        let stem = name.trim_end_matches(".mrpack").trim_end_matches(".zip");
        let path = cache.join(format!("{}-{stamp}.zip", loader::safe_name(stem)));
        if let Err(e) = download::fetch(url, &path, None, &plain) {
            eprintln!("failed to download modpack: {e}");
            std::process::exit(1);
        }
        (path, url.clone())
    } else if let Some(slug) = &c.modrinth {
        let path = cache.join(format!(
            "modpack-{}-{}.mrpack",
            loader::safe_name(slug),
            stamp
        ));
        match modrinth::download_modpack(slug, c.pack_version.as_deref(), &path) {
            Ok(version) => (path, format!("modrinth:{slug}@{version}")),
            Err(e) => {
                eprintln!("failed to download modpack: {e}");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("error: import requires --file, --url or --modrinth");
        std::process::exit(1);
    };

    let origins = download::Origins {
        mojang: c.mirror.as_deref(),
        maven: c.maven.as_deref(),
    };
    let loader_name = c.loader.as_deref().map(str::to_ascii_lowercase);
    let loader = match loader_name.as_deref() {
        Some("fabric") => Some(modpack::LoaderKind::Fabric),
        Some("forge") => Some(modpack::LoaderKind::Forge),
        Some("neoforge") => Some(modpack::LoaderKind::NeoForge),
        Some(other) => {
            eprintln!("unsupported loader: {other}");
            std::process::exit(1);
        }
        None => None,
    };

    let options = modpack::ImportOptions {
        mc_dir: &mc_dir,
        name: c.name.as_deref(),
        java: c.java.as_deref(),
        origins,
        with_optional: c.with_optional,
        dry_run: c.dry_run,
        source: &source,
        loader,
        loader_version: c.loader_version.as_deref(),
    };

    match modpack::import_file(&path, &options) {
        Ok(report) => {
            let action = if report.installed {
                "imported"
            } else {
                "dry run"
            };
            println!(
                "{} {} pack '{} {}'",
                action,
                report.provider.name(),
                report.name,
                report.version
            );
            println!("instance: {}", report.id);
            if !report.author.is_empty() {
                println!("author: {}", report.author);
            }
            if !report.description.is_empty() {
                println!("description: {}", report.description);
            }
            let loader = report
                .loader
                .as_ref()
                .map(|loader| {
                    format!(
                        "{} {}",
                        loader.kind.name(),
                        loader.version.as_deref().unwrap_or("")
                    )
                })
                .unwrap_or_else(|| String::from("vanilla"));
            println!("minecraft: {} / {}", report.game_version, loader);
            println!(
                "files: {}, overrides: {}, skipped optional: {}, size: {} MiB",
                report.files,
                report.overrides,
                report.excluded.len(),
                report.bytes / 1024 / 1024
            );
        }
        Err(e) => {
            eprintln!("failed to import modpack: {e}");
            std::process::exit(1);
        }
    }
}

fn run_launch(c: &LaunchCli) {
    let mc_dir = absolute_path(&c.mc_dir);
    let mc_dir = mc_dir.as_str();

    let resolved = version::resolve(mc_dir, &c.instance).unwrap_or_else(|error| {
        eprintln!("{}", error);
        std::process::exit(1);
    });
    let version = &resolved.json;

    let origins = download::Origins {
        mojang: c.mirror.as_deref(),
        maven: c.maven.as_deref(),
    };

    let java = launch::resolve_java(c.java.as_deref(), java::required_major_for_version(version));
    if let Err(e) = launch::check_java(&java, version) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let (username, uuid, access_token) = match resolve_identity(c) {
        Ok(identity) => identity,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = download::ensure_version_files(mc_dir, version, &resolved.jar_id, &origins) {
        eprintln!("failed to complete files: {}", e);
        std::process::exit(1);
    }
    if let Err(e) = download::ensure_assets(mc_dir, version, &origins) {
        eprintln!("failed to complete assets: {}", e);
        std::process::exit(1);
    }
    if let Err(e) = launch::prepare_natives(mc_dir, version, &resolved.jar_id) {
        eprintln!("failed to prepare natives: {}", e);
        std::process::exit(1);
    }

    let game_dir = instance::game_dir(mc_dir, &c.instance, c.isolate);
    if let Err(e) = std::fs::create_dir_all(&game_dir) {
        eprintln!("failed to create game directory {}: {}", game_dir, e);
        std::process::exit(1);
    }

    let opts = LaunchOptions {
        memory_mb: c.memory_mb,
        username,
        uuid,
        access_token,
        game_dir,
    };

    let args = build_args(mc_dir, version, &resolved.jar_id, &opts);

    let mut command = std::process::Command::new(&java);
    command.args(&args).current_dir(&opts.game_dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let status = command.status();

    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("game exited with {status}");
            std::process::exit(status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("failed to launch {java}: {e}");
            std::process::exit(1);
        }
    }
}

fn resolve_identity(c: &LaunchCli) -> Result<(String, String, String), String> {
    if let Some(name) = &c.username {
        return Ok((name.clone(), offline_uuid(name), String::from("0")));
    }

    if c.offline {
        return Err(String::from("--offline requires --username"));
    }

    if let Some(account) = microsoft::load_account() {
        if account.offline {
            return Ok((account.name, account.uuid, String::from("0")));
        }
        let mut account = account;
        microsoft::ensure_fresh(&mut account)?;
        return Ok((account.name, account.uuid, account.access_token));
    }

    Err(String::from(
        "no Microsoft account: run `lyys_mc_launcher login` first, or pass --username <name> for offline mode",
    ))
}

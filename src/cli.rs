use std::env;

pub enum Command {
    Launch(LaunchCli),
    Download(DownloadCli),
    Search(SearchCli),
    Add(AddCli),
    Import(ImportCli),
    Login,
    Logout,
    Mods(ModCli),
}

pub enum ModsAction {
    List,
    Remove,
    Toggle,
    Update,
}

pub struct ModCli {
    pub action: ModsAction,
    pub mc_dir: String,
    pub instance: String,
    pub mods: Vec<String>,
}

pub struct LaunchCli {
    pub mc_dir: String,
    pub instance: String,
    pub username: Option<String>,
    pub memory_mb: u32,
    pub isolate: bool,
    pub offline: bool,
    pub mirror: Option<String>,
    pub maven: Option<String>,
    pub java: Option<String>,
}

pub struct SearchCli {
    pub query: String,
    pub project_type: String,
    pub loaders: Vec<String>,
    pub versions: Vec<String>,
    pub limit: u32,
}

pub struct AddCli {
    pub mc_dir: String,
    pub instance: String,
    pub slug: String,
    pub project_type: String,
    pub with_optional: bool,
}

pub struct ImportCli {
    pub mc_dir: String,
    pub file: Option<String>,
    pub url: Option<String>,
    pub modrinth: Option<String>,
    pub pack_version: Option<String>,
    pub name: Option<String>,
    pub java: Option<String>,
    pub mirror: Option<String>,
    pub maven: Option<String>,
    pub with_optional: bool,
    pub dry_run: bool,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

pub struct DownloadCli {
    pub mc_dir: String,
    pub version: String,
    pub name: Option<String>,
    pub mirror: Option<String>,
    pub maven: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub java: Option<String>,
}

const USAGE: &str = concat!(
    "usage:\n",
    "  launch: lyys_mc_launcher launch --dir <game_dir> --name <instance> [--username <name>] [--memory <MB>] [--isolate] [--offline] [--mirror <url>] [--maven <url>] [--java <path>]\n",
    "  download: lyys_mc_launcher download --dir <game_dir> --version <version> [--loader <fabric|forge|neoforge>] [--loader-version <v>] [--name <instance>] [--java <path>] [--mirror <url>] [--maven <url>]\n",
    "  search: lyys_mc_launcher search --query <text> --type <mod|modpack|resourcepack|shader|datapack> [--loader <a,b>] [--version <x,y>] [--limit <n>]\n",
    "  add:    lyys_mc_launcher add --dir <game_dir> --name <instance> --slug <slug> --type <kind> [--with-optional]\n",
    "  import: lyys_mc_launcher import --dir <game_dir> (--file <archive> | --url <url> | --modrinth <slug>) [--pack-version <id>] [--name <instance>] [--java <path>] [--loader <fabric|forge|neoforge>] [--loader-version <v>] [--with-optional] [--dry-run] [--mirror <url>] [--maven <url>]\n",
    "  mods:   lyys_mc_launcher mods list|remove|toggle|update --dir <game_dir> --name <instance> [--mod <id,file,...>]\n",
    "  login:  lyys_mc_launcher login (Microsoft device code flow)\n",
    "  logout: lyys_mc_launcher logout",
);

fn reject_unknown_flags(rest: &[String], known: &[(&str, bool)]) {
    let mut index = 0;
    while index < rest.len() {
        let arg = rest[index].as_str();
        if !arg.starts_with('-') {
            eprintln!("error: unexpected argument: {arg}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
        let Some((_, takes_value)) = known.iter().find(|(flag, _)| *flag == arg) else {
            eprintln!("error: unknown flag: {arg}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        };
        index += if *takes_value { 2 } else { 1 };
    }
}

pub fn parse() -> Command {
    let args: Vec<String> = env::args().collect();
    let rest = &args[args.len().min(2)..];

    match args.get(1).map(String::as_str) {
        Some("launch") => Command::Launch(parse_launch(rest)),
        Some("download") => Command::Download(parse_download(rest)),
        Some("search") => Command::Search(parse_search(rest)),
        Some("add") => Command::Add(parse_add(rest)),
        Some("import") => Command::Import(parse_import(rest)),
        Some("login") => Command::Login,
        Some("logout") => Command::Logout,
        Some("mods") => Command::Mods(parse_mods(rest)),
        _ => {
            println!("{USAGE}");
            std::process::exit(0);
        }
    }
}

fn parse_launch(rest: &[String]) -> LaunchCli {
    reject_unknown_flags(
        rest,
        &[
            ("--dir", true),
            ("--name", true),
            ("--username", true),
            ("--memory", true),
            ("--mirror", true),
            ("--maven", true),
            ("--java", true),
            ("--isolate", false),
            ("--offline", false),
        ],
    );

    let mut mc_dir: Option<String> = None;
    let mut instance: Option<String> = None;
    let mut username: Option<String> = None;
    let mut memory_mb = 4096;
    let mut mirror: Option<String> = None;
    let mut maven: Option<String> = None;
    let mut java: Option<String> = None;

    for w in rest.windows(2) {
        match w[0].as_str() {
            "--dir" => mc_dir = Some(w[1].clone()),
            "--name" => instance = Some(w[1].clone()),
            "--username" => username = Some(w[1].clone()),
            "--memory" => memory_mb = w[1].parse().unwrap_or(memory_mb),
            "--mirror" => mirror = Some(w[1].clone()),
            "--maven" => maven = Some(w[1].clone()),
            "--java" => java = Some(w[1].clone()),
            _ => {}
        }
    }
    let isolate = rest.iter().any(|a| a == "--isolate");
    let offline = rest.iter().any(|a| a == "--offline");

    let mut missing = Vec::new();
    if mc_dir.is_none() {
        missing.push("--dir");
    }
    if instance.is_none() {
        missing.push("--name");
    }
    if !missing.is_empty() {
        eprintln!(
            "error: missing required argument(s): {}",
            missing.join(", ")
        );
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let mc_dir = mc_dir.unwrap();
    check_dir(&mc_dir);

    LaunchCli {
        mc_dir,
        instance: instance.unwrap(),
        username,
        memory_mb,
        isolate,
        offline,
        mirror,
        maven,
        java,
    }
}

fn parse_download(rest: &[String]) -> DownloadCli {
    reject_unknown_flags(
        rest,
        &[
            ("--dir", true),
            ("--version", true),
            ("--name", true),
            ("--mirror", true),
            ("--maven", true),
            ("--loader", true),
            ("--loader-version", true),
            ("--java", true),
        ],
    );

    let mut mc_dir: Option<String> = None;
    let mut version: Option<String> = None;
    let mut name: Option<String> = None;
    let mut mirror: Option<String> = None;
    let mut maven: Option<String> = None;
    let mut loader: Option<String> = None;
    let mut loader_version: Option<String> = None;
    let mut java: Option<String> = None;

    for w in rest.windows(2) {
        match w[0].as_str() {
            "--dir" => mc_dir = Some(w[1].clone()),
            "--version" => version = Some(w[1].clone()),
            "--name" => name = Some(w[1].clone()),
            "--mirror" => mirror = Some(w[1].clone()),
            "--maven" => maven = Some(w[1].clone()),
            "--loader" => loader = Some(w[1].clone()),
            "--loader-version" => loader_version = Some(w[1].clone()),
            "--java" => java = Some(w[1].clone()),
            _ => {}
        }
    }

    let (Some(mc_dir), Some(version)) = (mc_dir, version) else {
        eprintln!("error: download requires --dir and --version");
        eprintln!("{USAGE}");
        std::process::exit(1);
    };
    check_dir(&mc_dir);

    DownloadCli {
        mc_dir,
        version,
        name,
        mirror,
        maven,
        loader,
        loader_version,
        java,
    }
}

fn parse_search(rest: &[String]) -> SearchCli {
    reject_unknown_flags(
        rest,
        &[
            ("--query", true),
            ("--type", true),
            ("--loader", true),
            ("--version", true),
            ("--limit", true),
        ],
    );

    let mut query: Option<String> = None;
    let mut project_type: Option<String> = None;
    let mut loaders: Vec<String> = Vec::new();
    let mut versions: Vec<String> = Vec::new();
    let mut limit = 10;

    for w in rest.windows(2) {
        match w[0].as_str() {
            "--query" => query = Some(w[1].clone()),
            "--type" => project_type = Some(w[1].clone()),
            "--loader" => loaders.extend(split_list(&w[1])),
            "--version" => versions.extend(split_list(&w[1])),
            "--limit" => limit = w[1].parse().unwrap_or(limit),
            _ => {}
        }
    }

    let (Some(query), Some(project_type)) = (query, project_type) else {
        eprintln!("error: search requires --query and --type");
        eprintln!("{USAGE}");
        std::process::exit(1);
    };

    SearchCli {
        query,
        project_type,
        loaders,
        versions,
        limit,
    }
}

fn parse_add(rest: &[String]) -> AddCli {
    reject_unknown_flags(
        rest,
        &[
            ("--dir", true),
            ("--name", true),
            ("--slug", true),
            ("--type", true),
            ("--with-optional", false),
        ],
    );

    let mut mc_dir: Option<String> = None;
    let mut instance: Option<String> = None;
    let mut slug: Option<String> = None;
    let mut project_type: Option<String> = None;

    for w in rest.windows(2) {
        match w[0].as_str() {
            "--dir" => mc_dir = Some(w[1].clone()),
            "--name" => instance = Some(w[1].clone()),
            "--slug" => slug = Some(w[1].clone()),
            "--type" => project_type = Some(w[1].clone()),
            _ => {}
        }
    }
    let with_optional = rest.iter().any(|arg| arg == "--with-optional");

    let mut missing = Vec::new();
    if mc_dir.is_none() {
        missing.push("--dir");
    }
    if instance.is_none() {
        missing.push("--name");
    }
    if slug.is_none() {
        missing.push("--slug");
    }
    if project_type.is_none() {
        missing.push("--type");
    }
    if !missing.is_empty() {
        eprintln!(
            "error: missing required argument(s): {}",
            missing.join(", ")
        );
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let mc_dir = mc_dir.unwrap();
    check_dir(&mc_dir);

    AddCli {
        mc_dir,
        instance: instance.unwrap(),
        slug: slug.unwrap(),
        project_type: project_type.unwrap(),
        with_optional,
    }
}

fn parse_import(rest: &[String]) -> ImportCli {
    reject_unknown_flags(
        rest,
        &[
            ("--dir", true),
            ("--file", true),
            ("--url", true),
            ("--modrinth", true),
            ("--pack-version", true),
            ("--name", true),
            ("--java", true),
            ("--mirror", true),
            ("--maven", true),
            ("--loader", true),
            ("--loader-version", true),
            ("--with-optional", false),
            ("--dry-run", false),
        ],
    );

    let mut mc_dir: Option<String> = None;
    let mut file: Option<String> = None;
    let mut url: Option<String> = None;
    let mut modrinth: Option<String> = None;
    let mut pack_version: Option<String> = None;
    let mut name: Option<String> = None;
    let mut java: Option<String> = None;
    let mut mirror: Option<String> = None;
    let mut maven: Option<String> = None;
    let mut loader: Option<String> = None;
    let mut loader_version: Option<String> = None;

    for w in rest.windows(2) {
        match w[0].as_str() {
            "--dir" => mc_dir = Some(w[1].clone()),
            "--file" => file = Some(w[1].clone()),
            "--url" => url = Some(w[1].clone()),
            "--modrinth" => modrinth = Some(w[1].clone()),
            "--pack-version" => pack_version = Some(w[1].clone()),
            "--name" => name = Some(w[1].clone()),
            "--java" => java = Some(w[1].clone()),
            "--mirror" => mirror = Some(w[1].clone()),
            "--maven" => maven = Some(w[1].clone()),
            "--loader" => loader = Some(w[1].clone()),
            "--loader-version" => loader_version = Some(w[1].clone()),
            _ => {}
        }
    }

    let with_optional = rest.iter().any(|arg| arg == "--with-optional");
    let dry_run = rest.iter().any(|arg| arg == "--dry-run");

    let Some(mc_dir) = mc_dir else {
        eprintln!("error: import requires --dir");
        eprintln!("{USAGE}");
        std::process::exit(1);
    };

    let sources = [file.is_some(), url.is_some(), modrinth.is_some()]
        .into_iter()
        .filter(|present| *present)
        .count();
    if sources != 1 {
        eprintln!("error: import requires exactly one of --file, --url or --modrinth");
        eprintln!("{USAGE}");
        std::process::exit(1);
    }
    if pack_version.is_some() && modrinth.is_none() {
        eprintln!("error: --pack-version can only be used with --modrinth");
        std::process::exit(1);
    }
    check_dir(&mc_dir);

    ImportCli {
        mc_dir,
        file,
        url,
        modrinth,
        pack_version,
        name,
        java,
        mirror,
        maven,
        with_optional,
        dry_run,
        loader,
        loader_version,
    }
}

fn parse_mods(rest: &[String]) -> ModCli {
    let action = match rest.first().map(String::as_str) {
        Some("list") => ModsAction::List,
        Some("remove") => ModsAction::Remove,
        Some("toggle") => ModsAction::Toggle,
        Some("update") => ModsAction::Update,
        _ => {
            eprintln!("error: mods requires list, remove, toggle or update");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };

    let flags = rest.get(1..).unwrap_or(&[]);
    reject_unknown_flags(
        flags,
        &[("--dir", true), ("--name", true), ("--mod", true)],
    );

    let mut mc_dir: Option<String> = None;
    let mut instance: Option<String> = None;
    let mut mods: Vec<String> = Vec::new();

    for pair in flags.windows(2) {
        match pair[0].as_str() {
            "--dir" => mc_dir = Some(pair[1].clone()),
            "--name" => instance = Some(pair[1].clone()),
            "--mod" => mods.extend(split_list(&pair[1])),
            _ => {}
        }
    }

    let mut missing = Vec::new();
    if mc_dir.is_none() {
        missing.push("--dir");
    }
    if instance.is_none() {
        missing.push("--name");
    }
    if mods.is_empty() && matches!(action, ModsAction::Remove | ModsAction::Toggle) {
        missing.push("--mod");
    }
    if !missing.is_empty() {
        eprintln!(
            "error: missing required argument(s): {}",
            missing.join(", ")
        );
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let mc_dir = mc_dir.unwrap();
    check_dir(&mc_dir);

    ModCli {
        action,
        mc_dir,
        instance: instance.unwrap(),
        mods,
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn check_dir(mc_dir: &str) {
    if !std::path::Path::new(mc_dir).is_dir() {
        eprintln!("error: game directory does not exist: {mc_dir}");
        std::process::exit(1);
    }
}

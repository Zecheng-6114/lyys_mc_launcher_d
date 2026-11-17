use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

pub const JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };

#[derive(Clone, PartialEq, Eq)]
pub struct JavaRuntime {
    pub path: String,
    pub major: u32,
    pub arch: String,
    pub version: String,
}

#[derive(Clone)]
struct CacheEntry {
    key: String,
    major: u32,
    arch: String,
    version: String,
}

/// 游戏版本对应的最低 Java 主版本。
pub fn required_major(mc_version: &str) -> u32 {
    if version_at_least(mc_version, "26.1") {
        25
    } else if version_at_least(mc_version, "1.20.5") {
        21
    } else if version_at_least(mc_version, "1.18") {
        17
    } else if version_at_least(mc_version, "1.17") {
        16
    } else {
        8
    }
}

/// 优先使用版本 JSON 自带的 `javaVersion.majorVersion`，缺失时按游戏版本推断。
pub fn required_major_for_version(version: &Value) -> u32 {
    if let Some(major) = version["javaVersion"]["majorVersion"].as_u64() {
        return major as u32;
    }
    required_major(version["clientVersion"].as_str().unwrap_or_default())
}

/// 精确匹配优先，其次取大于需求的最低版本，最后回退到低于需求的最高版本。
pub fn select(runtimes: &[JavaRuntime], required: u32) -> Option<&JavaRuntime> {
    fn best<'a>(list: Vec<&'a JavaRuntime>) -> Option<&'a JavaRuntime> {
        list.into_iter()
            .max_by_key(|runtime| (is_64(&runtime.arch), std::cmp::Reverse(runtime.path.len())))
    }

    let exact: Vec<&JavaRuntime> = runtimes
        .iter()
        .filter(|runtime| runtime.major == required)
        .collect();
    if let Some(runtime) = best(exact) {
        return Some(runtime);
    }

    let above: Vec<&JavaRuntime> = runtimes
        .iter()
        .filter(|runtime| runtime.major > required)
        .collect();
    if let Some(lowest) = above.iter().map(|runtime| runtime.major).min() {
        let candidates = above
            .into_iter()
            .filter(|runtime| runtime.major == lowest)
            .collect();
        return best(candidates);
    }

    let below: Vec<&JavaRuntime> = runtimes
        .iter()
        .filter(|runtime| runtime.major < required)
        .collect();
    if let Some(highest) = below.iter().map(|runtime| runtime.major).max() {
        let candidates = below
            .into_iter()
            .filter(|runtime| runtime.major == highest)
            .collect();
        return best(candidates);
    }

    None
}

/// 扫描本机 Java；`force` 为真时忽略缓存重新探测。
pub fn detect(force: bool) -> Vec<JavaRuntime> {
    let cache = if force { HashMap::new() } else { load_cache() };
    let (runtimes, entries) = scan(&cache);
    save_cache(&entries);
    runtimes
}

/// 用户手动添加的 Java 路径（可执行文件、Java 主目录或包含多个 JDK 的父目录）。
pub fn user_paths() -> Vec<String> {
    read_config()["user_java"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn add_user_path(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(String::from("empty Java path"));
    }

    let candidate = Path::new(path);
    if !candidate.exists() {
        return Err(format!("path does not exist: {path}"));
    }

    let value = normalize(candidate);
    let mut paths = user_paths();
    if !paths
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        paths.push(value);
    }
    write_user_paths(&paths)
}

pub fn remove_user_path(path: &str) -> Result<(), String> {
    let target = normalize(Path::new(path.trim()));
    let mut paths = user_paths();
    paths.retain(|existing| !existing.eq_ignore_ascii_case(&target));
    write_user_paths(&paths)
}

pub fn major_of(java: &str) -> Result<u32, String> {
    let output = command(java)
        .arg("-version")
        .output()
        .map_err(|e| format!("failed to run {java}: {e}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    let start = text
        .find("version \"")
        .map(|index| index + "version \"".len())
        .ok_or_else(|| format!("failed to detect Java version from {java}"))?;
    let version = text[start..].split('"').next().unwrap_or("");

    version_major(version)
        .ok_or_else(|| format!("failed to parse Java version: {version}"))
}

#[cfg(windows)]
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;

    let mut command = Command::new(program);
    command.creation_flags(0x0800_0000);
    command
}

#[cfg(not(windows))]
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

fn scan(cache: &HashMap<String, CacheEntry>) -> (Vec<JavaRuntime>, HashMap<String, CacheEntry>) {
    let mut seen = HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for path in candidates() {
        let display = normalize(&path);
        let key = if cfg!(windows) {
            display.to_ascii_lowercase()
        } else {
            display.clone()
        };
        if seen.insert(key) {
            unique.push(display);
        }
    }

    let mut entries = HashMap::new();
    let mut runtimes = Vec::new();
    let mut pending: Vec<(String, String)> = Vec::new();

    for path in unique {
        let Some(fingerprint) = fingerprint(Path::new(&path)) else {
            continue;
        };

        match cache.get(&path) {
            Some(entry) if entry.key == fingerprint => {
                runtimes.push(JavaRuntime {
                    path: path.clone(),
                    major: entry.major,
                    arch: entry.arch.clone(),
                    version: entry.version.clone(),
                });
                entries.insert(path, entry.clone());
            }
            _ => pending.push((path, fingerprint)),
        }
    }

    let total = pending.len();
    let next = AtomicUsize::new(0);
    let collected: Mutex<Vec<(String, String, JavaRuntime)>> = Mutex::new(Vec::new());
    let workers = total.clamp(1, 8);

    if total > 0 {
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= total {
                        break;
                    }
                    let (path, fingerprint) = &pending[index];
                    if let Some(runtime) = probe(Path::new(path)) {
                        if let Ok(mut list) = collected.lock() {
                            list.push((path.clone(), fingerprint.clone(), runtime));
                        }
                    }
                });
            }
        });
    }

    for (path, fingerprint, runtime) in collected.into_inner().unwrap_or_default() {
        entries.insert(
            path,
            CacheEntry {
                key: fingerprint,
                major: runtime.major,
                arch: runtime.arch.clone(),
                version: runtime.version.clone(),
            },
        );
        runtimes.push(runtime);
    }

    runtimes.sort_by(|left, right| {
        right
            .major
            .cmp(&left.major)
            .then(is_64(&right.arch).cmp(&is_64(&left.arch)))
            .then(left.path.cmp(&right.path))
    });
    runtimes.dedup_by(|left, right| left.path.eq_ignore_ascii_case(&right.path));
    (runtimes, entries)
}

/// 用可执行文件与 `release` 文件的大小、修改时间生成缓存键，JDK 更新后自动失效。
fn fingerprint(java: &Path) -> Option<String> {
    let meta = std::fs::metadata(java).ok()?;
    if !meta.is_file() {
        return None;
    }

    let mut key = format!("sz:{},m:{}", meta.len(), modified_millis(&meta));
    if let Some(home) = java.parent().and_then(Path::parent) {
        if let Ok(release) = std::fs::metadata(home.join("release")) {
            key.push_str(&format!(
                ",rsz:{},rm:{}",
                release.len(),
                modified_millis(&release)
            ));
        }
    }
    Some(key)
}

fn modified_millis(meta: &std::fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn probe(java: &Path) -> Option<JavaRuntime> {
    if !java.is_file() {
        return None;
    }

    let output = command(java)
        .args(["-XshowSettings:properties", "-version"])
        .output()
        .ok()?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    let version = property(&text, "java.version").unwrap_or_default();
    let arch = property(&text, "os.arch").unwrap_or_default();
    let major = version_major(&version).or_else(|| major_of(&java.to_string_lossy()).ok())?;

    Some(JavaRuntime {
        path: normalize(java),
        major,
        arch: arch.to_string(),
        version: version.to_string(),
    })
}

fn property<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        return Some(value.trim());
    }
    None
}

fn version_major(version: &str) -> Option<u32> {
    let version = version.split(['-', '+']).next()?.trim();
    if version.is_empty() {
        return None;
    }
    let rest = version.strip_prefix("1.").unwrap_or(version);
    rest.split(['.', '_']).next()?.parse().ok()
}

fn version_at_least(version: &str, minimum: &str) -> bool {
    let Some(left) = version_numbers(version) else {
        return false;
    };
    let Some(right) = version_numbers(minimum) else {
        return false;
    };

    for index in 0..left.len().max(right.len()) {
        let left = left.get(index).copied().unwrap_or(0);
        let right = right.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    true
}

fn version_numbers(version: &str) -> Option<Vec<u64>> {
    let prefix: String = version
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let prefix = prefix.trim_end_matches('.');

    if prefix.is_empty() {
        return None;
    }

    prefix
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u64>>>()
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    for key in ["JAVA_HOME", "JDK_HOME"] {
        if let Some(home) = std::env::var_os(key) {
            out.push(home_executable(Path::new(&home)));
        }
    }

    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            out.push(entry.join(JAVA_BIN));
        }
    }

    for entry in user_paths() {
        let path = PathBuf::from(entry);
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() {
            out.push(home_executable(&path));
            walk_bin(&path, 4, &mut out);
        }
    }

    #[cfg(windows)]
    windows_candidates(&mut out);
    #[cfg(target_os = "macos")]
    macos_candidates(&mut out);
    #[cfg(target_os = "linux")]
    linux_candidates(&mut out);

    out
}

fn home_executable(home: &Path) -> PathBuf {
    if home.is_file() {
        return home.to_path_buf();
    }
    home.join("bin").join(JAVA_BIN)
}

#[cfg(windows)]
fn windows_candidates(out: &mut Vec<PathBuf>) {
    const VENDORS: &[&str] = &[
        "Java",
        "Eclipse Adoptium",
        "Eclipse Foundation",
        "Microsoft",
        "Zulu",
        "BellSoft",
        "Amazon Corretto",
        "GraalVM",
        "Semeru",
        "OpenJDK",
        "jdk",
    ];

    for root in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramW6432"),
    ]
    .into_iter()
    .flatten()
    {
        for vendor in VENDORS {
            let base = Path::new(&root).join(vendor);
            out.push(home_executable(&base));
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        out.push(home_executable(&path));
                    }
                }
            }
        }
    }

    if let Some(user) = std::env::var_os("USERPROFILE") {
        for dir in [".jdks", ".gradle\\jdks"] {
            let base = Path::new(&user).join(dir);
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    out.push(home_executable(&entry.path()));
                }
            }
        }
    }

    if let Some(appdata) = std::env::var_os("APPDATA") {
        walk_bin(&Path::new(&appdata).join(".minecraft").join("runtime"), 4, out);
    }

    for home in registry_homes() {
        out.push(home_executable(&home));
    }
}

#[cfg(windows)]
fn registry_homes() -> Vec<PathBuf> {
    const KEYS: &[&str] = &[
        r"HKLM\SOFTWARE\JavaSoft\Java Runtime Environment",
        r"HKLM\SOFTWARE\JavaSoft\Java Development Kit",
        r"HKLM\SOFTWARE\JavaSoft\JRE",
        r"HKLM\SOFTWARE\JavaSoft\JDK",
        r"HKLM\SOFTWARE\Eclipse Adoptium\JDK",
        r"HKLM\SOFTWARE\Eclipse Adoptium\JRE",
        r"HKLM\SOFTWARE\Eclipse Foundation\JDK",
        r"HKLM\SOFTWARE\Microsoft\JDK",
    ];

    let mut queries: Vec<(&str, &str)> = Vec::new();
    for key in KEYS {
        for view in ["/reg:64", "/reg:32"] {
            queries.push((key, view));
        }
    }

    let homes: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for &(key, view) in &queries {
            scope.spawn(|| {
                let Ok(output) = command("reg")
                    .args(["query", key, "/s", view])
                    .output()
                else {
                    return;
                };
                if !output.status.success() {
                    return;
                }

                let mut found = Vec::new();
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let line = line.trim();
                    let Some((name, rest)) = line.split_once(|c: char| c.is_whitespace()) else {
                        continue;
                    };
                    if !matches!(name, "JavaHome" | "InstallationPath" | "Path") {
                        continue;
                    }

                    let rest = rest.trim_start();
                    let Some(value) = rest
                        .strip_prefix("REG_SZ")
                        .or_else(|| rest.strip_prefix("REG_EXPAND_SZ"))
                    else {
                        continue;
                    };

                    let value = value.trim();
                    if !value.is_empty() {
                        found.push(PathBuf::from(value));
                    }
                }

                if let Ok(mut all) = homes.lock() {
                    all.extend(found);
                }
            });
        }
    });

    homes.into_inner().unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn linux_candidates(out: &mut Vec<PathBuf>) {
    let mut roots = vec![
        PathBuf::from("/usr/lib/jvm"),
        PathBuf::from("/usr/lib64/jvm"),
        PathBuf::from("/usr/java"),
        PathBuf::from("/opt/jdk"),
        PathBuf::from("/opt/jdks"),
    ];

    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        roots.push(home.join(".sdkman").join("candidates").join("java"));
        roots.push(home.join(".jdks"));
        roots.push(home.join(".gradle").join("jdks"));
    }

    for root in roots {
        out.push(home_executable(&root));
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                out.push(home_executable(&entry.path()));
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_candidates(out: &mut Vec<PathBuf>) {
    let mut roots = vec![PathBuf::from("/Library/Java/JavaVirtualMachines")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(Path::new(&home).join("Library/Java/JavaVirtualMachines"));
    }

    for root in roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                out.push(
                    entry
                        .path()
                        .join("Contents")
                        .join("Home")
                        .join("bin")
                        .join(JAVA_BIN),
                );
            }
        }
    }

    for prefix in ["/opt/homebrew/opt", "/usr/local/opt"] {
        if let Ok(entries) = std::fs::read_dir(prefix) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("openjdk") {
                    out.push(
                        entry
                            .path()
                            .join("libexec")
                            .join("openjdk.jdk")
                            .join("Contents")
                            .join("Home")
                            .join("bin")
                            .join(JAVA_BIN),
                    );
                }
            }
        }
    }
}

fn walk_bin(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let home_java = path.join("bin").join(JAVA_BIN);
        if home_java.is_file() {
            out.push(home_java);
            continue;
        }

        if path.join(JAVA_BIN).is_file() {
            out.push(path.join(JAVA_BIN));
            continue;
        }

        walk_bin(&path, depth - 1, out);
    }
}

fn normalize(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = canonical.to_string_lossy().to_string();
    text.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(text)
}

fn is_64(arch: &str) -> bool {
    arch.contains("64")
}

fn cache_path() -> PathBuf {
    crate::loader::base_dir()
        .join("lyys_mc_launcher")
        .join("java.json")
}

fn config_path() -> PathBuf {
    crate::loader::base_dir()
        .join("lyys_mc_launcher")
        .join("config.json")
}

fn read_config() -> Value {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_user_paths(paths: &[String]) -> Result<(), String> {
    let mut config = read_config();
    if !config.is_object() {
        config = json!({});
    }
    config["user_java"] = json!(paths);

    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

fn load_cache() -> HashMap<String, CacheEntry> {
    let mut entries = HashMap::new();
    let Ok(text) = std::fs::read_to_string(cache_path()) else {
        return entries;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return entries;
    };
    let Some(runtimes) = value["runtimes"].as_object() else {
        return entries;
    };

    for (path, item) in runtimes {
        let Some(key) = item["key"].as_str() else {
            continue;
        };
        entries.insert(
            path.clone(),
            CacheEntry {
                key: key.to_string(),
                major: item["major"].as_u64().unwrap_or_default() as u32,
                arch: item["arch"].as_str().unwrap_or_default().to_string(),
                version: item["version"].as_str().unwrap_or_default().to_string(),
            },
        );
    }
    entries
}

fn save_cache(entries: &HashMap<String, CacheEntry>) {
    let runtimes = entries
        .iter()
        .map(|(path, entry)| {
            (
                path.clone(),
                json!({
                    "key": entry.key,
                    "major": entry.major,
                    "arch": entry.arch,
                    "version": entry.version,
                }),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
    let value = json!({ "runtimes": runtimes });

    let path = cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(major: u32, arch: &str, path: &str) -> JavaRuntime {
        JavaRuntime {
            path: path.to_string(),
            major,
            arch: arch.to_string(),
            version: String::new(),
        }
    }

    #[test]
    fn requirement_follows_game_version() {
        assert_eq!(required_major("1.12.2"), 8);
        assert_eq!(required_major("1.16.5"), 8);
        assert_eq!(required_major("1.17.1"), 16);
        assert_eq!(required_major("1.18.2"), 17);
        assert_eq!(required_major("1.20.4"), 17);
        assert_eq!(required_major("1.20.5"), 21);
        assert_eq!(required_major("1.21.4"), 21);
        assert_eq!(required_major("26.1"), 25);
        assert_eq!(required_major("23w45a"), 21);
    }

    #[test]
    fn requirement_prefers_version_json() {
        let version = json!({"clientVersion": "1.20.4", "javaVersion": {"majorVersion": 21}});
        assert_eq!(required_major_for_version(&version), 21);
        let version = json!({"clientVersion": "1.17.1"});
        assert_eq!(required_major_for_version(&version), 16);
    }

    #[test]
    fn selection_prefers_exact_then_closest_above() {
        let runtimes = vec![
            runtime(8, "amd64", "java8"),
            runtime(17, "amd64", "java17"),
            runtime(21, "amd64", "java21"),
        ];

        assert_eq!(select(&runtimes, 17).unwrap().path, "java17");
        assert_eq!(select(&runtimes, 16).unwrap().path, "java17");
        assert_eq!(select(&runtimes, 21).unwrap().path, "java21");
        assert_eq!(select(&runtimes, 25).unwrap().path, "java21");
        assert_eq!(select(&runtimes, 11).unwrap().path, "java17");
    }

    #[test]
    fn selection_prefers_64_bit_on_tie() {
        let runtimes = vec![
            runtime(17, "x86", "java17-32"),
            runtime(17, "x86_64", "java17-64"),
        ];
        assert_eq!(select(&runtimes, 17).unwrap().path, "java17-64");
    }

    #[test]
    fn parses_legacy_and_modern_versions() {
        assert_eq!(version_major("1.8.0_402"), Some(8));
        assert_eq!(version_major("8.0.402"), Some(8));
        assert_eq!(version_major("17.0.10"), Some(17));
        assert_eq!(version_major("21"), Some(21));
        assert_eq!(version_major("21.0.1+12"), Some(21));
        assert_eq!(version_major(""), None);
    }
}

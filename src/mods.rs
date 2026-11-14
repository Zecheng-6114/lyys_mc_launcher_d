use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::download::{self, FileTask, Origins};
use crate::installer;
use crate::instance;
use crate::loader;
use crate::modrinth;
use crate::version;

const DISABLED_SUFFIX: &str = ".jar.disabled";
const CACHE_FILE: &str = "mods-cache.json";
const SCAN_CACHE_FILE: &str = "mods-scan-cache.json";

#[derive(Clone)]
pub struct ModEntry {
    pub file: String,
    pub id: String,
    pub version: String,
    pub disabled: bool,
    pub source: Option<String>,
}

#[derive(Clone)]
struct LocalEntry {
    file: String,
    id: String,
    version: String,
    disabled: bool,
    sha1: Option<String>,
}

struct CachedMod {
    source: String,
    version: String,
}

struct ScannedMod {
    size: u64,
    mtime: u64,
    id: String,
    version: String,
    sha1: String,
}

pub fn mods_dir(mc_dir: &str, instance: &str) -> PathBuf {
    Path::new(&instance::game_dir(mc_dir, instance, false)).join("mods")
}

pub fn list(mc_dir: &str, instance: &str) -> Result<Vec<ModEntry>, String> {
    list_with_progress(mc_dir, instance, |_| {})
}

pub fn list_with_progress(
    mc_dir: &str,
    instance: &str,
    notify: impl Fn(&[ModEntry]),
) -> Result<Vec<ModEntry>, String> {
    let local = scan(mc_dir, instance)?;
    let mut entries: Vec<ModEntry> = local
        .iter()
        .map(|entry| ModEntry {
            file: entry.file.clone(),
            id: entry.id.clone(),
            version: entry.version.clone(),
            disabled: entry.disabled,
            source: None,
        })
        .collect();
    notify(&entries);

    enrich(&local, &mut entries);
    notify(&entries);
    Ok(entries)
}

fn enrich(local: &[LocalEntry], entries: &mut [ModEntry]) {
    let mut cache = read_cache();
    let missing: Vec<String> = local
        .iter()
        .filter_map(|entry| entry.sha1.clone())
        .filter(|hash| !cache.contains_key(hash))
        .collect();
    if !missing.is_empty() {
        refresh_cache(&missing, &mut cache);
    }

    for (index, entry) in local.iter().enumerate() {
        let Some(cached) = entry.sha1.as_deref().and_then(|hash| cache.get(hash)) else {
            continue;
        };
        if !cached.source.is_empty() {
            entries[index].source = Some(cached.source.clone());
        }
        if entries[index].version.is_empty() && !cached.version.is_empty() {
            entries[index].version = cached.version.clone();
        }
    }
}

fn refresh_cache(hashes: &[String], cache: &mut HashMap<String, CachedMod>) {
    let Ok(versions) = modrinth::versions_by_hashes(hashes) else {
        return;
    };

    let mut ids: Vec<String> = versions
        .values()
        .filter_map(|version| version["project_id"].as_str().map(str::to_string))
        .collect();
    ids.sort();
    ids.dedup();
    let titles = modrinth::projects_by_ids(&ids).ok();

    for hash in hashes {
        let Some(version) = versions.get(hash) else {
            cache.insert(
                hash.clone(),
                CachedMod {
                    source: String::new(),
                    version: String::new(),
                },
            );
            continue;
        };
        let project = version["project_id"].as_str().unwrap_or_default();
        let source = titles
            .as_ref()
            .and_then(|projects| projects.as_array())
            .and_then(|projects| {
                projects
                    .iter()
                    .find(|entry| entry["id"].as_str() == Some(project))
            })
            .and_then(|entry| entry["title"].as_str())
            .unwrap_or_default()
            .to_string();
        let number = version["version_number"].as_str().unwrap_or_default();
        cache.insert(
            hash.clone(),
            CachedMod {
                source,
                version: number.to_string(),
            },
        );
    }

    write_cache(cache);
}

fn cache_path() -> PathBuf {
    loader::base_dir()
        .join("lyys_mc_launcher")
        .join(CACHE_FILE)
}

fn scan_cache_path() -> PathBuf {
    loader::base_dir()
        .join("lyys_mc_launcher")
        .join(SCAN_CACHE_FILE)
}

fn read_scan_cache() -> HashMap<String, ScannedMod> {
    let mut cache = HashMap::new();
    let Ok(text) = fs::read_to_string(scan_cache_path()) else {
        return cache;
    };
    let Ok(Value::Object(entries)) = serde_json::from_str::<Value>(&text) else {
        return cache;
    };
    for (file, entry) in entries {
        cache.insert(
            file,
            ScannedMod {
                size: entry["size"].as_u64().unwrap_or_default(),
                mtime: entry["mtime"].as_u64().unwrap_or_default(),
                id: entry["id"].as_str().unwrap_or_default().to_string(),
                version: entry["version"].as_str().unwrap_or_default().to_string(),
                sha1: entry["sha1"].as_str().unwrap_or_default().to_string(),
            },
        );
    }
    cache
}

fn write_scan_cache(cache: &HashMap<String, ScannedMod>) {
    let entries: Map<String, Value> = cache
        .iter()
        .filter(|(_, entry)| !entry.id.is_empty())
        .map(|(file, entry)| {
            (
                file.clone(),
                json!({
                    "size": entry.size,
                    "mtime": entry.mtime,
                    "id": entry.id,
                    "version": entry.version,
                    "sha1": entry.sha1,
                }),
            )
        })
        .collect();
    let path = scan_cache_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&path, Value::Object(entries).to_string());
}

fn read_cache() -> HashMap<String, CachedMod> {
    let mut cache = HashMap::new();
    let Ok(text) = fs::read_to_string(cache_path()) else {
        return cache;
    };
    let Ok(Value::Object(entries)) = serde_json::from_str::<Value>(&text) else {
        return cache;
    };
    for (hash, entry) in entries {
        cache.insert(
            hash,
            CachedMod {
                source: entry["source"].as_str().unwrap_or_default().to_string(),
                version: entry["version"].as_str().unwrap_or_default().to_string(),
            },
        );
    }
    cache
}

fn write_cache(cache: &HashMap<String, CachedMod>) {
    let entries: Map<String, Value> = cache
        .iter()
        .map(|(hash, entry)| {
            (
                hash.clone(),
                json!({ "source": entry.source, "version": entry.version }),
            )
        })
        .collect();
    let path = cache_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&path, Value::Object(entries).to_string());
}

pub fn remove(mc_dir: &str, instance: &str, names: &[String]) -> Result<usize, String> {
    let local = scan(mc_dir, instance)?;
    let dir = mods_dir(mc_dir, instance);

    let mut targets: Vec<LocalEntry> = Vec::new();
    for name in names {
        let Some(entry) = local
            .iter()
            .find(|entry| entry.file == *name || entry.id == *name)
        else {
            return Err(format!("mod not found: {name}"));
        };
        if targets.iter().all(|target| target.file != entry.file) {
            targets.push(entry.clone());
        }
    }

    for entry in &targets {
        let path = dir.join(&entry.file);
        fs::remove_file(&path).map_err(|e| format!("failed to remove {}: {e}", path.display()))?;
        println!("removed {}", entry.file);
    }

    Ok(targets.len())
}

pub fn toggle(mc_dir: &str, instance: &str, name: &str) -> Result<bool, String> {
    let local = scan(mc_dir, instance)?;
    let entry = local
        .iter()
        .find(|entry| entry.file == name || entry.id == name)
        .ok_or_else(|| format!("mod not found: {name}"))?;

    let dir = mods_dir(mc_dir, instance);
    let from = dir.join(&entry.file);
    let to = if entry.disabled {
        let enabled_name = entry
            .file
            .strip_suffix(".disabled")
            .unwrap_or(&entry.file);
        dir.join(enabled_name)
    } else {
        dir.join(format!("{}.disabled", entry.file))
    };
    fs::rename(&from, &to).map_err(|e| format!("failed to rename {}: {e}", from.display()))?;

    let state = if entry.disabled {
        "enabled"
    } else {
        "disabled"
    };
    let renamed = to
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| entry.file.clone());
    println!("{state} {renamed}");

    Ok(!entry.disabled)
}

pub fn update(mc_dir: &str, instance: &str) -> Result<usize, String> {
    let local = scan(mc_dir, instance)?;
    let dir = mods_dir(mc_dir, instance);

    let updatable: Vec<LocalEntry> = local
        .iter()
        .filter(|entry| !entry.disabled && entry.sha1.is_some())
        .cloned()
        .collect();
    if updatable.is_empty() {
        println!("no mods to update");
        return Ok(0);
    }

    let info = version::instance_info(mc_dir, instance)?;
    let loaders: Vec<String> = info.loader.into_iter().collect();
    let hashes: Vec<String> = updatable
        .iter()
        .map(|entry| entry.sha1.clone().unwrap_or_default())
        .collect();

    let response = modrinth::update_versions(&hashes, &loaders, &[info.mc_version])?;

    let mut tasks = Vec::new();
    let mut plan: Vec<(String, String)> = Vec::new();
    for entry in &updatable {
        let hash = entry.sha1.as_deref().unwrap_or_default();
        let Some(update) = response.get(hash) else {
            println!("skipping {} (not on modrinth)", entry.file);
            continue;
        };
        let files = update["files"].as_array().cloned().unwrap_or_default();
        let file = files
            .iter()
            .find(|file| file["primary"].as_bool() == Some(true))
            .or_else(|| files.first());
        let Some(file) = file else {
            println!("skipping {} (no update file)", entry.file);
            continue;
        };
        let sha1 = file["hashes"]["sha1"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if sha1.is_empty() || sha1 == hash {
            println!("skipping {} (already latest)", entry.file);
            continue;
        }
        let Some(url) = file["url"].as_str() else {
            println!("skipping {} (no update url)", entry.file);
            continue;
        };
        let Some(filename) = file["filename"].as_str() else {
            println!("skipping {} (no update filename)", entry.file);
            continue;
        };
        if filename.is_empty() || filename.contains(['/', '\\']) || filename.contains("..") {
            return Err(format!("unsafe mod filename: {filename}"));
        }

        tasks.push(FileTask {
            urls: vec![url.to_string()],
            dest: dir.join(filename),
            size: file["size"].as_u64(),
            sha1: Some(sha1),
            sha512: file["hashes"]["sha512"].as_str().map(str::to_string),
        });
        plan.push((entry.file.clone(), filename.to_string()));
    }

    if tasks.is_empty() {
        println!("all mods up to date");
        return Ok(0);
    }

    let plain = Origins {
        mojang: None,
        maven: None,
    };
    download::download_all(tasks, &plain)?;

    for (old, new) in &plan {
        if old != new {
            let path = dir.join(old);
            fs::remove_file(&path)
                .map_err(|e| format!("failed to remove {}: {e}", path.display()))?;
        }
        println!("updated {old} -> {new}");
    }

    Ok(plan.len())
}

fn scan(mc_dir: &str, instance: &str) -> Result<Vec<LocalEntry>, String> {
    let dir = mods_dir(mc_dir, instance);
    let read = match fs::read_dir(&dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("failed to read {}: {e}", dir.display())),
    };

    struct Pending {
        file: String,
        path: PathBuf,
        size: u64,
        mtime: u64,
        disabled: bool,
    }

    let mut cache = read_scan_cache();
    let mut entries = Vec::new();
    let mut pending: Vec<Pending> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for item in read {
        let item = item.map_err(|e| format!("failed to read {}: {e}", dir.display()))?;
        let path = item.path();
        let meta = match item.metadata() {
            Ok(meta) if meta.is_file() => meta,
            _ => continue,
        };
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let disabled = file.ends_with(DISABLED_SUFFIX);
        if !disabled && !file.ends_with(".jar") {
            continue;
        }

        let size = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();

        seen.push(file.clone());
        if let Some(cached) = cache.get(&file)
            && cached.size == size
            && cached.mtime == mtime
            && !cached.id.is_empty()
        {
            entries.push(LocalEntry {
                file,
                id: cached.id.clone(),
                version: cached.version.clone(),
                disabled,
                sha1: (!cached.sha1.is_empty()).then(|| cached.sha1.clone()),
            });
            continue;
        }

        pending.push(Pending {
            file,
            path,
            size,
            mtime,
            disabled,
        });
    }

    let mut dirty = false;
    if !pending.is_empty() {
        let workers = pending.len().min(8);
        let per_worker = pending.len().div_ceil(workers);
        let parsed = std::thread::scope(|scope| {
            let handles: Vec<_> = pending
                .chunks(per_worker)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|item| {
                                let (id, version) = parse_metadata(&item.path)
                                    .unwrap_or_else(|| (fallback_id(&item.file), String::new()));
                                let sha1 = download::file_sha1(&item.path).unwrap_or_default();
                                (item, id, version, sha1)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap_or_default())
                .collect::<Vec<_>>()
        });

        for (item, id, version, sha1) in parsed {
            cache.insert(
                item.file.clone(),
                ScannedMod {
                    size: item.size,
                    mtime: item.mtime,
                    id: id.clone(),
                    version: version.clone(),
                    sha1: sha1.clone(),
                },
            );
            entries.push(LocalEntry {
                file: item.file.clone(),
                id,
                version,
                disabled: item.disabled,
                sha1: (!sha1.is_empty()).then_some(sha1),
            });
        }
        dirty = true;
    }

    if cache.len() != seen.len() {
        cache.retain(|file, _| seen.iter().any(|name| name == file));
        dirty = true;
    }
    if dirty {
        write_scan_cache(&cache);
    }

    entries.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(entries)
}

fn parse_metadata(path: &Path) -> Option<(String, String)> {
    if let Some((id, version)) = fabric_metadata(path) {
        return Some((id, version));
    }
    if let Some((id, version)) = quilt_metadata(path) {
        return Some((id, version));
    }
    forge_metadata(path)
}

fn fabric_metadata(path: &Path) -> Option<(String, String)> {
    let text = installer::read_zip_text(path, "fabric.mod.json").ok()??;
    let value: Value = serde_json::from_str(&text).ok()?;
    let id = value["id"].as_str().filter(|id| !id.is_empty())?.to_string();
    let version = value["version"].as_str().unwrap_or_default().to_string();
    Some((id, version))
}

fn quilt_metadata(path: &Path) -> Option<(String, String)> {
    let text = installer::read_zip_text(path, "quilt.mod.json").ok()??;
    let value: Value = serde_json::from_str(&text).ok()?;
    let loader = &value["quilt_loader"];
    let id = loader["id"]
        .as_str()
        .filter(|id| !id.is_empty())?
        .to_string();
    let version = loader["version"].as_str().unwrap_or_default().to_string();
    Some((id, version))
}

fn forge_metadata(path: &Path) -> Option<(String, String)> {
    let text = installer::read_zip_text(path, "META-INF/mods.toml")
        .ok()
        .flatten()
        .or_else(|| {
            installer::read_zip_text(path, "META-INF/neoforge.mods.toml")
                .ok()
                .flatten()
        })?;
    parse_mods_toml(&text)
}

fn parse_mods_toml(text: &str) -> Option<(String, String)> {
    let mut id = None;
    let mut version = None;
    let mut in_mods = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("[[mods]]") {
            if id.is_some() {
                break;
            }
            in_mods = true;
            continue;
        }
        if line.starts_with('[') {
            in_mods = false;
            continue;
        }
        if !in_mods {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = toml_value(value);
            match key.trim() {
                "modId" if id.is_none() => id = Some(value),
                "version" if version.is_none() => version = Some(value),
                _ => {}
            }
        }
    }

    let id = id.filter(|id| !id.is_empty())?;
    let version = match version {
        Some(value) if !value.starts_with("${") => value,
        _ => String::new(),
    };
    Some((id, version))
}

fn toml_value(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(quote)
            && let Some((inner, _)) = inner.rsplit_once(quote)
        {
            return inner.to_string();
        }
    }
    value
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn fallback_id(name: &str) -> String {
    name.trim_end_matches(DISABLED_SUFFIX)
        .trim_end_matches(".jar")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mods_toml_reads_first_mod_section() {
        let text = "modLoader=\"javafml\"\nloaderVersion=\"[40,)\"\n\n[[mods]]\nmodId=\"jei\" # comment\nversion=\"15.2.0\"\ndisplayName=\"JEI\"\n\n[[dependencies.jei]]\nmodId=\"minecraft\"\n";
        assert_eq!(
            parse_mods_toml(text).unwrap(),
            (String::from("jei"), String::from("15.2.0"))
        );
    }

    #[test]
    fn parse_mods_toml_ignores_second_mod_section() {
        let text = "[[mods]]\nmodId=\"first\"\nversion=\"1.0\"\n[[mods]]\nmodId=\"second\"\nversion=\"2.0\"\n";
        assert_eq!(
            parse_mods_toml(text).unwrap(),
            (String::from("first"), String::from("1.0"))
        );
    }

    #[test]
    fn parse_mods_toml_treats_jar_template_as_unknown() {
        let text = "[[mods]]\nmodId=\"legacy\"\nversion=\"${file.jarVersion}\"\n";
        assert_eq!(
            parse_mods_toml(text).unwrap(),
            (String::from("legacy"), String::new())
        );
    }

    #[test]
    fn parse_mods_toml_requires_mod_id() {
        assert!(parse_mods_toml("license=\"MIT\"\nversion=\"1.0\"\n").is_none());
    }

    #[test]
    fn fallback_id_strips_disabled_and_jar_suffix() {
        assert_eq!(fallback_id("jei.jar"), "jei");
        assert_eq!(fallback_id("jei.jar.disabled"), "jei");
    }

    #[test]
    fn toml_value_strips_quotes_and_comments() {
        assert_eq!(toml_value("\"1.20.1\""), "1.20.1");
        assert_eq!(toml_value("'lit'"), "lit");
        assert_eq!(toml_value("1.0 # trailing"), "1.0");
        assert_eq!(toml_value("\"jei\" # comment"), "jei");
    }
}

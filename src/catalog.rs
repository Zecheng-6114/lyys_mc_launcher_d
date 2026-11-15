use std::collections::HashSet;

use serde_json::Value;

use crate::download::{self, Origins};
use crate::metadata;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersion {
    pub id: String,
    pub kind: String,
}

pub fn game_versions(origins: &Origins) -> Result<Vec<GameVersion>, String> {
    let manifest = download::fetch_json(download::MOJANG_MANIFEST, origins)?;
    parse_game_versions(&manifest)
}

pub fn loader_versions(
    loader: &str,
    mc_version: &str,
    origins: &Origins,
) -> Result<Vec<String>, String> {
    match loader {
        "fabric" => {
            let url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{}",
                download::encode_component(mc_version)
            );
            let json = download::fetch_json(&url, origins)?;
            parse_fabric_versions(&json)
        }
        "forge" => metadata::forge_versions(mc_version, origins),
        "neoforge" => metadata::neoforge_versions(mc_version, origins),
        other => Err(format!("unsupported loader: {other}")),
    }
}

fn parse_game_versions(manifest: &Value) -> Result<Vec<GameVersion>, String> {
    let entries = manifest["versions"]
        .as_array()
        .ok_or_else(|| String::from("manifest has no versions"))?;

    let versions: Vec<GameVersion> = entries
        .iter()
        .filter_map(|entry| {
            Some(GameVersion {
                id: entry["id"].as_str()?.to_string(),
                kind: entry["type"].as_str().unwrap_or("release").to_string(),
            })
        })
        .collect();

    if versions.is_empty() {
        return Err(String::from("manifest has no versions"));
    }

    Ok(versions)
}

fn parse_fabric_versions(json: &Value) -> Result<Vec<String>, String> {
    let entries = json
        .as_array()
        .ok_or_else(|| String::from("Fabric meta returned no loader versions"))?;

    let versions: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry["loader"]["version"].as_str().map(str::to_string))
        .collect();
    let versions = dedup(versions);

    if versions.is_empty() {
        return Err(String::from("no Fabric loader versions found"));
    }

    Ok(versions)
}

fn dedup(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_manifest_versions_in_order() {
        let manifest = json!({
            "versions": [
                { "id": "1.21.2", "type": "release" },
                { "id": "24w40a", "type": "snapshot" },
                { "id": "b1.7.3", "type": "old_beta" }
            ]
        });
        assert_eq!(
            parse_game_versions(&manifest).unwrap(),
            vec![
                GameVersion {
                    id: String::from("1.21.2"),
                    kind: String::from("release"),
                },
                GameVersion {
                    id: String::from("24w40a"),
                    kind: String::from("snapshot"),
                },
                GameVersion {
                    id: String::from("b1.7.3"),
                    kind: String::from("old_beta"),
                },
            ]
        );
    }

    #[test]
    fn rejects_manifest_without_versions() {
        assert!(parse_game_versions(&json!({})).is_err());
        assert!(parse_game_versions(&json!({ "versions": [] })).is_err());
    }

    #[test]
    fn parses_fabric_versions() {
        let json = json!([
            { "loader": { "version": "0.16.10" } },
            { "loader": { "version": "0.16.9" } },
            { "loader": { "version": "0.16.10" } }
        ]);
        assert_eq!(
            parse_fabric_versions(&json).unwrap(),
            vec!["0.16.10", "0.16.9"]
        );
    }

    #[test]
    fn rejects_missing_fabric_versions() {
        assert!(parse_fabric_versions(&json!({ "error": "not found" })).is_err());
        assert!(parse_fabric_versions(&json!([])).is_err());
    }
}

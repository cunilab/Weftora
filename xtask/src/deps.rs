use crate::{FORBIDDEN, FORBIDDEN_PREFIXES, HEADLESS, command_output, files};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
    process::Command,
};

pub fn run(root: &Path, offline: bool) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--locked"]);
    if offline {
        command.arg("--offline");
    }
    let metadata: Value = serde_json::from_slice(&command_output(&mut command)?)
        .map_err(|error| format!("invalid cargo metadata: {error}"))?;
    let mut problems = check_graph(&metadata)?;
    for path in files(root)? {
        if path.file_name().is_some_and(|name| name == "Cargo.toml")
            && under_examples(path.strip_prefix(root).map_err(|error| error.to_string())?)
        {
            problems.push(format!(
                "{}: Story examples must not contain Cargo.toml",
                path.display()
            ));
        }
    }
    if problems.is_empty() {
        println!("dependency rules passed");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn under_examples(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "examples")
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("cargo metadata missing {key} array"))
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("cargo metadata missing {key} string"))
}

fn check_graph(metadata: &Value) -> Result<Vec<String>, String> {
    let packages = array(metadata, "packages")?;
    let mut names = HashMap::new();
    let mut forbidden = HashSet::new();
    let root = Path::new(field(metadata, "workspace_root")?);
    let members = array(metadata, "workspace_members")?;
    for package in packages {
        let id = field(package, "id")?;
        let name = field(package, "name")?;
        names.insert(id, name);
        let manifest = Path::new(field(package, "manifest_path")?);
        let example = manifest.strip_prefix(root).is_ok_and(under_examples);
        if FORBIDDEN.contains(&name)
            || FORBIDDEN_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
            || example
        {
            forbidden.insert(id);
        }
    }
    let mut edges = HashMap::new();
    for node in array(&metadata["resolve"], "nodes")? {
        let id = field(node, "id")?;
        let mut dependencies = Vec::new();
        for dependency in array(node, "deps")? {
            // A target-specific dependency still counts. Only exclusively dev edges are ignored.
            if array(dependency, "dep_kinds")?.iter().any(|kind| {
                kind["kind"].is_null()
                    || kind["kind"].as_str() == Some("normal")
                    || kind["kind"].as_str() == Some("build")
            }) {
                let next = field(dependency, "pkg")?;
                if !names.contains_key(next) {
                    return Err(format!("cargo metadata references unknown package {next}"));
                }
                dependencies.push(next);
            }
        }
        edges.insert(id, dependencies);
    }
    let mut problems = Vec::new();
    for headless in HEADLESS {
        let start = names
            .iter()
            .find_map(|(&id, &name)| {
                (name == *headless && members.iter().any(|member| member.as_str() == Some(id)))
                    .then_some(id)
            })
            .ok_or_else(|| format!("missing headless workspace package {headless}"))?;
        let mut visited = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, vec![start])]);
        while let Some((id, path)) = queue.pop_front() {
            let dependencies = edges
                .get(id)
                .ok_or_else(|| format!("missing resolve node for {id}"))?;
            for &next in dependencies {
                if !visited.insert(next) {
                    continue;
                }
                let mut next_path = path.clone();
                next_path.push(next);
                if forbidden.contains(next) {
                    let path = next_path
                        .iter()
                        .map(|id| format!("{} ({id})", names[id]))
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    problems.push(format!("{headless}: forbidden dependency path: {path}"));
                }
                queue.push_back((next, next_path));
            }
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        serde_json::from_str(r#"{
            "workspace_root":"/repo",
            "workspace_members":["api","core","story"],
            "packages":[
                {"id":"api","name":"weftora-api","manifest_path":"/repo/crates/api/Cargo.toml"},
                {"id":"core","name":"weftora-core","manifest_path":"/repo/crates/core/Cargo.toml"},
                {"id":"story","name":"weftora-story","manifest_path":"/repo/crates/story/Cargo.toml"},
                {"id":"helper","name":"helper","manifest_path":"/registry/helper/Cargo.toml"}
            ],
            "resolve":{"nodes":[
                {"id":"api","deps":[]},
                {"id":"core","deps":[{"pkg":"helper","dep_kinds":[{"kind":null}]}]},
                {"id":"story","deps":[{"pkg":"api","dep_kinds":[{"kind":null}]}]},
                {"id":"helper","deps":[]}
            ]}
        }"#).unwrap()
    }
    #[test]
    fn clean_graph() {
        assert!(check_graph(&fixture()).unwrap().is_empty());
    }
    #[test]
    fn transitive_bevy() {
        let mut graph = fixture();
        graph["packages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id":"bevy","name":"bevy_ecs","manifest_path":"/registry/bevy/Cargo.toml"
            }));
        graph["resolve"]["nodes"][3]["deps"] = serde_json::json!([
            {"pkg":"bevy","dep_kinds":[{"kind":"build"}]}
        ]);
        graph["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":"bevy","deps":[]}));
        let problems = check_graph(&graph).unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("weftora-core (core) -> helper (helper) -> bevy_ecs (bevy)"));
    }
    #[test]
    fn example_dependency() {
        let mut graph = fixture();
        graph["packages"][3]["manifest_path"] = "/repo/examples/hello/Cargo.toml".into();
        assert!(check_graph(&graph).unwrap()[0].contains("weftora-core"));
    }
    #[test]
    fn dev_dependencies_ignored_but_mixed_edges_checked() {
        let mut graph = fixture();
        graph["packages"][3]["name"] = "rhai_codegen".into();
        graph["resolve"]["nodes"][1]["deps"][0]["dep_kinds"] = serde_json::json!([{"kind":"dev"}]);
        assert!(check_graph(&graph).unwrap().is_empty());
        graph["resolve"]["nodes"][1]["deps"][0]["dep_kinds"] =
            serde_json::json!([{"kind":"dev"},{"kind":"build"}]);
        assert_eq!(check_graph(&graph).unwrap().len(), 1);
    }
}

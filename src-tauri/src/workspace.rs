use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{HermesEnvironment, HermesRequest, KeyValueRow, TreeNode, WorkspaceConfig},
};

const WORKSPACE_FILE: &str = "hermes.workspace.yaml";
const COLLECTIONS_DIR: &str = "collections";
const ENVIRONMENTS_DIR: &str = "environments";

pub fn create_workspace(path: String, name: String) -> Result<WorkspaceConfig> {
    let root = PathBuf::from(path);
    fs::create_dir_all(root.join(COLLECTIONS_DIR))?;
    fs::create_dir_all(root.join(ENVIRONMENTS_DIR))?;
    fs::create_dir_all(root.join(".hermes"))?;

    let config = WorkspaceConfig {
        id: Uuid::new_v4().to_string(),
        name,
        version: 1,
    };
    write_yaml(root.join(WORKSPACE_FILE), &config)?;

    let env = HermesEnvironment {
        id: "local".to_string(),
        name: "Local".to_string(),
        values: vec![
            KeyValueRow {
                id: "baseUrl".to_string(),
                key: "baseUrl".to_string(),
                value: "https://api.example.com".to_string(),
                enabled: true,
                secret: false,
            },
            KeyValueRow {
                id: "token".to_string(),
                key: "token".to_string(),
                value: "".to_string(),
                enabled: true,
                secret: true,
            },
        ],
    };
    write_yaml(root.join(ENVIRONMENTS_DIR).join("local.yaml"), &env)?;
    Ok(config)
}

pub fn open_workspace(path: String) -> Result<WorkspaceConfig> {
    read_workspace_config(path)
}

pub fn read_workspace_config(workspace_path: String) -> Result<WorkspaceConfig> {
    read_yaml(PathBuf::from(workspace_path).join(WORKSPACE_FILE))
}

pub fn list_workspace_tree(workspace_path: String) -> Result<Vec<TreeNode>> {
    let root = PathBuf::from(workspace_path);
    let collections = root.join(COLLECTIONS_DIR);
    if !collections.exists() {
        return Ok(Vec::new());
    }
    build_tree(&root, &collections)
}

pub fn read_request(workspace_path: String, request_path: String) -> Result<HermesRequest> {
    read_request_from_path(&safe_join(&PathBuf::from(workspace_path), &request_path)?)
}

pub fn write_request(
    workspace_path: String,
    request_path: String,
    request: HermesRequest,
) -> Result<String> {
    let root = PathBuf::from(workspace_path);
    let path = safe_join(&root, &request_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_yaml(&path, &request)?;
    rel_to_workspace(&root, &path)
}

pub fn delete_request(workspace_path: String, request_path: String) -> Result<()> {
    let path = safe_join(&PathBuf::from(workspace_path), &request_path)?;
    if path.is_dir() {
        return Err(AppError::user(
            "invalid_path",
            "Only request files can be deleted.",
        ));
    }
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn duplicate_request(workspace_path: String, request_path: String) -> Result<String> {
    let root = PathBuf::from(workspace_path);
    let source = safe_join(&root, &request_path)?;
    let parent = source
        .parent()
        .ok_or_else(|| AppError::user("invalid_path", "Request has no parent folder."))?;
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("request");
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("yaml");

    for index in 1..1000 {
        let candidate = parent.join(format!("{stem}-copy-{index}.{extension}"));
        if !candidate.exists() {
            fs::copy(&source, &candidate)?;
            let mut request = read_request_from_path(&candidate)?;
            request.id = Uuid::new_v4().to_string();
            request.name = format!("{} copy {}", request.name, index);
            write_yaml(&candidate, &request)?;
            return rel_to_workspace(&root, &candidate);
        }
    }

    Err(AppError::user(
        "duplicate_failed",
        "Could not find a free duplicate file name.",
    ))
}

pub fn create_group(workspace_path: String, group_path: String) -> Result<String> {
    let root = PathBuf::from(workspace_path);
    let group = normalize_group_path(&group_path)?;
    let path = safe_join(&root, &format!("{COLLECTIONS_DIR}/{group}"))?;
    fs::create_dir_all(&path)?;
    rel_to_workspace(&root, &path)
}

pub fn delete_group(workspace_path: String, group_path: String, recursive: bool) -> Result<()> {
    let root = PathBuf::from(workspace_path);
    let group = normalize_group_path(&group_path)?;
    let path = safe_join(&root, &format!("{COLLECTIONS_DIR}/{group}"))?;
    if !path.is_dir() {
        return Err(AppError::user(
            "invalid_group",
            "Group folder does not exist.",
        ));
    }
    if recursive {
        fs::remove_dir_all(path)?;
        return Ok(());
    }

    fs::remove_dir(path).map_err(|err| {
        AppError::with_detail(
            "group_not_empty",
            "Group is not empty. Confirm deletion to remove it and all contained requests.",
            err.to_string(),
        )
    })?;
    Ok(())
}

pub fn rename_group(
    workspace_path: String,
    group_path: String,
    new_group_path: String,
) -> Result<String> {
    let root = PathBuf::from(workspace_path);
    let group = normalize_group_path(&group_path)?;
    let new_group = normalize_group_path(&new_group_path)?;
    if group == new_group {
        return Ok(format!("{COLLECTIONS_DIR}/{group}"));
    }
    if new_group.starts_with(&format!("{group}/")) {
        return Err(AppError::user(
            "invalid_group",
            "A group cannot be moved inside itself.",
        ));
    }

    let source = safe_join(&root, &format!("{COLLECTIONS_DIR}/{group}"))?;
    let destination = safe_join(&root, &format!("{COLLECTIONS_DIR}/{new_group}"))?;
    if !source.is_dir() {
        return Err(AppError::user(
            "invalid_group",
            "Group folder does not exist.",
        ));
    }
    if destination.exists() {
        return Err(AppError::user(
            "group_exists",
            "A group with this name already exists.",
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, &destination)?;
    rel_to_workspace(&root, &destination)
}

pub fn list_environments(workspace_path: String) -> Result<Vec<HermesEnvironment>> {
    let env_dir = PathBuf::from(workspace_path).join(ENVIRONMENTS_DIR);
    if !env_dir.exists() {
        return Ok(Vec::new());
    }

    let mut environments = Vec::new();
    for entry in sorted_entries(&env_dir)? {
        let path = entry.path();
        if is_yaml(&path) {
            environments.push(read_yaml(path)?);
        }
    }
    Ok(environments)
}

pub fn read_environment(
    workspace_path: String,
    environment_id: String,
) -> Result<HermesEnvironment> {
    read_yaml(
        PathBuf::from(workspace_path)
            .join(ENVIRONMENTS_DIR)
            .join(format!("{environment_id}.yaml")),
    )
}

pub fn write_environment(workspace_path: String, environment: HermesEnvironment) -> Result<String> {
    let env_dir = PathBuf::from(workspace_path).join(ENVIRONMENTS_DIR);
    fs::create_dir_all(&env_dir)?;
    let id = environment.id.clone();
    write_yaml(env_dir.join(format!("{id}.yaml")), &environment)?;
    Ok(id)
}

pub fn read_request_from_path(path: &Path) -> Result<HermesRequest> {
    read_yaml(path.to_path_buf())
}

pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(AppError::user(
            "invalid_path",
            "Workspace paths must be relative.",
        ));
    }
    for component in relative_path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        ) {
            return Err(AppError::user(
                "invalid_path",
                "Workspace path escapes the workspace root.",
            ));
        }
    }
    Ok(root.join(relative_path))
}

fn build_tree(root: &Path, dir: &Path) -> Result<Vec<TreeNode>> {
    let mut nodes = Vec::new();
    for entry in sorted_entries(dir)? {
        let path = entry.path();
        if path.is_dir() {
            let children = build_tree(root, &path)?;
            nodes.push(TreeNode::Folder {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: rel_to_workspace(root, &path)?,
                children,
            });
        } else if is_yaml(&path) {
            let request = read_request_from_path(&path).map_err(|err| {
                AppError::with_detail(
                    "invalid_request_file",
                    format!("Could not read request file {}", path.display()),
                    err.to_string(),
                )
            })?;
            nodes.push(TreeNode::Request {
                name: request.name,
                path: rel_to_workspace(root, &path)?,
                method: request.method,
                id: request.id,
            });
        }
    }
    Ok(nodes)
}

fn sorted_entries(path: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

fn is_yaml(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("yaml" | "yml")
    )
}

fn normalize_group_path(group_path: &str) -> Result<String> {
    let group = group_path
        .trim()
        .trim_matches('/')
        .trim_start_matches("collections/")
        .trim_matches('/');
    if group.is_empty() {
        return Err(AppError::user(
            "invalid_group",
            "Group name cannot be empty.",
        ));
    }
    Ok(group.to_string())
}

fn read_yaml<T: serde::de::DeserializeOwned>(path: PathBuf) -> Result<T> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&contents)?)
}

fn write_yaml<T: serde::Serialize>(path: impl AsRef<Path>, value: &T) -> Result<()> {
    let contents = serde_yaml::to_string(value)?;
    fs::write(path, contents)?;
    Ok(())
}

fn rel_to_workspace(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| AppError::user("invalid_path", "Path is outside workspace."))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HttpMethod;

    #[test]
    fn safe_join_rejects_parent_paths() {
        let root = PathBuf::from("/workspace");
        assert!(safe_join(&root, "../secret.yaml").is_err());
    }

    #[test]
    fn workspace_roundtrip_creates_expected_files() {
        let dir = tempfile::tempdir().unwrap();
        let config =
            create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string()).unwrap();
        assert_eq!(config.name, "Test");
        assert!(dir.path().join(WORKSPACE_FILE).exists());
        assert!(dir.path().join(COLLECTIONS_DIR).exists());
        assert!(dir
            .path()
            .join(ENVIRONMENTS_DIR)
            .join("local.yaml")
            .exists());
    }

    #[test]
    fn request_yaml_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string()).unwrap();
        let request = HermesRequest {
            id: "req".to_string(),
            name: "Ping".to_string(),
            method: HttpMethod::Get,
            url: "https://example.com".to_string(),
            params: Vec::new(),
            headers: Vec::new(),
            body: Default::default(),
            auth: Default::default(),
            description: None,
        };
        let path = write_request(
            dir.path().to_string_lossy().to_string(),
            "collections/ping.yaml".to_string(),
            request,
        )
        .unwrap();
        assert_eq!(path, "collections/ping.yaml");
        let tree = list_workspace_tree(dir.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn creates_empty_group_folder() {
        let dir = tempfile::tempdir().unwrap();
        create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string()).unwrap();
        let path = create_group(
            dir.path().to_string_lossy().to_string(),
            "auth/admin".to_string(),
        )
        .unwrap();
        assert_eq!(path, "collections/auth/admin");
        assert!(dir.path().join("collections/auth/admin").exists());
    }

    #[test]
    fn recursively_deletes_non_empty_group() {
        let dir = tempfile::tempdir().unwrap();
        create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string()).unwrap();
        create_group(dir.path().to_string_lossy().to_string(), "auth".to_string()).unwrap();
        fs::write(dir.path().join("collections/auth/note.txt"), "temporary").unwrap();

        delete_group(
            dir.path().to_string_lossy().to_string(),
            "auth".to_string(),
            true,
        )
        .unwrap();

        assert!(!dir.path().join("collections/auth").exists());
    }

    #[test]
    fn renames_group_with_contents() {
        let dir = tempfile::tempdir().unwrap();
        create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string()).unwrap();
        create_group(dir.path().to_string_lossy().to_string(), "auth/admin".to_string()).unwrap();
        fs::write(
            dir.path().join("collections/auth/admin/note.txt"),
            "temporary",
        )
        .unwrap();

        let path = rename_group(
            dir.path().to_string_lossy().to_string(),
            "auth/admin".to_string(),
            "auth/users".to_string(),
        )
        .unwrap();

        assert_eq!(path, "collections/auth/users");
        assert!(!dir.path().join("collections/auth/admin").exists());
        assert!(dir.path().join("collections/auth/users/note.txt").exists());
    }
}

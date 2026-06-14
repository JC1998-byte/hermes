use std::{collections::BTreeSet, fs, path::PathBuf};

use keyring::Entry;

use crate::{error::Result, models::SecretMetadata, workspace};

const KEYCHAIN_SERVICE: &str = "dev.hermes.api-client";

pub fn set_secret(
    workspace_path: &str,
    environment_id: &str,
    key: &str,
    value: &str,
) -> Result<()> {
    let account = account_name(workspace_path, environment_id, key)?;
    Entry::new(KEYCHAIN_SERVICE, &account)?.set_password(value)?;
    let mut metadata = read_metadata(workspace_path, environment_id)?;
    let mut keys = metadata.keys.into_iter().collect::<BTreeSet<_>>();
    keys.insert(key.to_string());
    metadata.keys = keys.into_iter().collect();
    write_metadata(workspace_path, environment_id, &metadata)
}

pub fn get_secret(workspace_path: &str, environment_id: &str, key: &str) -> Result<String> {
    let account = account_name(workspace_path, environment_id, key)?;
    Ok(Entry::new(KEYCHAIN_SERVICE, &account)?.get_password()?)
}

pub fn get_secret_metadata(workspace_path: &str, environment_id: &str) -> Result<Vec<String>> {
    Ok(read_metadata(workspace_path, environment_id)?.keys)
}

pub fn delete_secret(workspace_path: &str, environment_id: &str, key: &str) -> Result<()> {
    let account = account_name(workspace_path, environment_id, key)?;
    let _ = Entry::new(KEYCHAIN_SERVICE, &account)?.delete_credential();
    let mut metadata = read_metadata(workspace_path, environment_id)?;
    metadata.keys.retain(|item| item != key);
    write_metadata(workspace_path, environment_id, &metadata)
}

pub fn account_name(workspace_path: &str, environment_id: &str, key: &str) -> Result<String> {
    let workspace = workspace::read_workspace_config(workspace_path.to_string())?;
    Ok(format!("{}:{}:{}", workspace.id, environment_id, key))
}

fn metadata_path(workspace_path: &str, environment_id: &str) -> PathBuf {
    PathBuf::from(workspace_path)
        .join(".hermes")
        .join(format!("secrets-{environment_id}.yaml"))
}

fn read_metadata(workspace_path: &str, environment_id: &str) -> Result<SecretMetadata> {
    let path = metadata_path(workspace_path, environment_id);
    if !path.exists() {
        return Ok(SecretMetadata { keys: Vec::new() });
    }
    Ok(serde_yaml::from_str(&fs::read_to_string(path)?)?)
}

fn write_metadata(
    workspace_path: &str,
    environment_id: &str,
    metadata: &SecretMetadata,
) -> Result<()> {
    let path = metadata_path(workspace_path, environment_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(metadata)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_name_contains_workspace_id() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = workspace::create_workspace(
            dir.path().to_string_lossy().to_string(),
            "Test".to_string(),
        )
        .unwrap();
        let account = account_name(&dir.path().to_string_lossy(), "local", "token").unwrap();
        assert!(account.starts_with(&workspace.id));
        assert!(account.ends_with(":local:token"));
    }
}

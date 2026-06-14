use std::collections::HashMap;

use regex::Regex;

use crate::{
    error::{AppError, Result},
    secrets, workspace,
};

pub fn resolve_text(
    workspace_path: &str,
    environment_id: Option<&str>,
    text: &str,
) -> Result<String> {
    let regex = Regex::new(r"\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}").expect("valid variable regex");
    if !regex.is_match(text) {
        return Ok(text.to_string());
    }

    let mut referenced = regex
        .captures_iter(text)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .collect::<Vec<_>>();
    referenced.sort();
    referenced.dedup();

    let env_id = environment_id.ok_or_else(|| {
        AppError::user(
            "missing_environment",
            "This request contains variables but no environment is selected.",
        )
    })?;
    let environment = workspace::read_environment(workspace_path.to_string(), env_id.to_string())?;
    let mut values = HashMap::new();

    for row in environment.values.iter().filter(|row| {
        row.enabled && !row.key.trim().is_empty() && referenced.iter().any(|key| key == &row.key)
    }) {
        let value = if row.secret {
            secrets::get_secret(workspace_path, env_id, &row.key).map_err(|err| {
                AppError::with_detail(
                    "missing_secret",
                    format!(
                        "Secret '{}' is referenced but not available in the OS keychain.",
                        row.key
                    ),
                    err.to_string(),
                )
            })?
        } else {
            row.value.clone()
        };
        values.insert(row.key.clone(), value);
    }

    let mut unresolved = Vec::new();
    let resolved = regex.replace_all(text, |captures: &regex::Captures<'_>| {
        let key = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        match values.get(key) {
            Some(value) => value.to_string(),
            None => {
                unresolved.push(key.to_string());
                captures.get(0).unwrap().as_str().to_string()
            }
        }
    });

    if !unresolved.is_empty() {
        unresolved.sort();
        unresolved.dedup();
        return Err(AppError::user(
            "unresolved_variable",
            format!("Unresolved variables: {}", unresolved.join(", ")),
        ));
    }

    Ok(resolved.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_environment_values() {
        let dir = tempfile::tempdir().unwrap();
        workspace::create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string())
            .unwrap();
        let result = resolve_text(
            &dir.path().to_string_lossy(),
            Some("local"),
            "{{baseUrl}}/users",
        )
        .unwrap();
        assert_eq!(result, "https://api.example.com/users");
    }

    #[test]
    fn fails_on_unresolved_values() {
        let dir = tempfile::tempdir().unwrap();
        workspace::create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string())
            .unwrap();
        assert!(resolve_text(&dir.path().to_string_lossy(), Some("local"), "{{missing}}").is_err());
    }
}

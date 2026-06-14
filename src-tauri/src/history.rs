use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use crate::{
    error::{AppError, Result},
    models::HistoryEntry,
};

pub fn append_history(workspace_path: &str, entry: &HistoryEntry) -> Result<()> {
    let path = history_path(workspace_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

pub fn list_history(workspace_path: &str) -> Result<Vec<HistoryEntry>> {
    let path = history_path(workspace_path);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for line in fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        entries.push(serde_json::from_str::<HistoryEntry>(line)?);
    }
    entries.reverse();
    Ok(entries)
}

pub fn read_history_entry(workspace_path: &str, id: &str) -> Result<HistoryEntry> {
    list_history(workspace_path)?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| AppError::user("history_not_found", "History entry was not found."))
}

pub fn clear_history(workspace_path: &str) -> Result<()> {
    let path = history_path(workspace_path);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn history_path(workspace_path: &str) -> PathBuf {
    PathBuf::from(workspace_path)
        .join(".hermes")
        .join("history.jsonl")
}

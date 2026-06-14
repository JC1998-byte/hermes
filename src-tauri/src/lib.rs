mod error;
mod history;
mod http_client;
mod imports;
mod models;
mod secrets;
mod variables;
mod workspace;

use error::Result;
use models::*;

#[tauri::command]
fn create_workspace(path: String, name: String) -> Result<WorkspaceConfig> {
    workspace::create_workspace(path, name)
}

#[tauri::command]
fn open_workspace(path: String) -> Result<WorkspaceConfig> {
    workspace::open_workspace(path)
}

#[tauri::command]
fn list_workspace_tree(workspace_path: String) -> Result<Vec<TreeNode>> {
    workspace::list_workspace_tree(workspace_path)
}

#[tauri::command]
fn read_workspace_config(workspace_path: String) -> Result<WorkspaceConfig> {
    workspace::read_workspace_config(workspace_path)
}

#[tauri::command]
fn read_request(workspace_path: String, request_path: String) -> Result<HermesRequest> {
    workspace::read_request(workspace_path, request_path)
}

#[tauri::command]
fn write_request(
    workspace_path: String,
    request_path: String,
    request: HermesRequest,
) -> Result<String> {
    workspace::write_request(workspace_path, request_path, request)
}

#[tauri::command]
fn delete_request(workspace_path: String, request_path: String) -> Result<()> {
    workspace::delete_request(workspace_path, request_path)
}

#[tauri::command]
fn duplicate_request(workspace_path: String, request_path: String) -> Result<String> {
    workspace::duplicate_request(workspace_path, request_path)
}

#[tauri::command]
fn create_group(workspace_path: String, group_path: String) -> Result<String> {
    workspace::create_group(workspace_path, group_path)
}

#[tauri::command]
fn delete_group(workspace_path: String, group_path: String, recursive: bool) -> Result<()> {
    workspace::delete_group(workspace_path, group_path, recursive)
}

#[tauri::command]
fn rename_group(
    workspace_path: String,
    group_path: String,
    new_group_path: String,
) -> Result<String> {
    workspace::rename_group(workspace_path, group_path, new_group_path)
}

#[tauri::command]
async fn send_request(input: SendRequestInput) -> Result<HermesResponse> {
    http_client::send_request(input).await
}

#[tauri::command]
fn list_environments(workspace_path: String) -> Result<Vec<HermesEnvironment>> {
    workspace::list_environments(workspace_path)
}

#[tauri::command]
fn read_environment(workspace_path: String, environment_id: String) -> Result<HermesEnvironment> {
    workspace::read_environment(workspace_path, environment_id)
}

#[tauri::command]
fn write_environment(workspace_path: String, environment: HermesEnvironment) -> Result<String> {
    workspace::write_environment(workspace_path, environment)
}

#[tauri::command]
fn resolve_variables(
    workspace_path: String,
    text: String,
    environment_id: Option<String>,
) -> Result<String> {
    variables::resolve_text(&workspace_path, environment_id.as_deref(), &text)
}

#[tauri::command]
fn set_secret(
    workspace_path: String,
    environment_id: String,
    key: String,
    value: String,
) -> Result<()> {
    secrets::set_secret(&workspace_path, &environment_id, &key, &value)
}

#[tauri::command]
fn get_secret_metadata(workspace_path: String, environment_id: String) -> Result<Vec<String>> {
    secrets::get_secret_metadata(&workspace_path, &environment_id)
}

#[tauri::command]
fn delete_secret(workspace_path: String, environment_id: String, key: String) -> Result<()> {
    secrets::delete_secret(&workspace_path, &environment_id, &key)
}

#[tauri::command]
fn import_preview(kind: String, payload: String) -> Result<ImportPreview> {
    imports::preview(&kind, &payload)
}

#[tauri::command]
fn import_collection(
    workspace_path: String,
    kind: String,
    payload: String,
) -> Result<ImportResult> {
    imports::import_collection(&workspace_path, &kind, &payload)
}

#[tauri::command]
fn import_curl(workspace_path: String, command_text: String) -> Result<ImportResult> {
    imports::import_collection(&workspace_path, "curl", &command_text)
}

#[tauri::command]
fn import_postman_collection(workspace_path: String, payload: String) -> Result<ImportResult> {
    imports::import_collection(&workspace_path, "postman", &payload)
}

#[tauri::command]
fn import_openapi(workspace_path: String, payload: String) -> Result<ImportResult> {
    imports::import_collection(&workspace_path, "openapi", &payload)
}

#[tauri::command]
fn list_history(workspace_path: String) -> Result<Vec<HistoryEntry>> {
    history::list_history(&workspace_path)
}

#[tauri::command]
fn read_history_entry(workspace_path: String, id: String) -> Result<HistoryEntry> {
    history::read_history_entry(&workspace_path, &id)
}

#[tauri::command]
fn clear_history(workspace_path: String) -> Result<()> {
    history::clear_history(&workspace_path)
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            create_workspace,
            open_workspace,
            list_workspace_tree,
            read_workspace_config,
            read_request,
            write_request,
            delete_request,
            duplicate_request,
            create_group,
            delete_group,
            rename_group,
            send_request,
            list_environments,
            read_environment,
            write_environment,
            resolve_variables,
            set_secret,
            get_secret_metadata,
            delete_secret,
            import_preview,
            import_collection,
            import_curl,
            import_postman_collection,
            import_openapi,
            list_history,
            read_history_entry,
            clear_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hermes");
}

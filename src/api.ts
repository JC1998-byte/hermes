import { invoke } from "@tauri-apps/api/core";
import type {
  HermesEnvironment,
  HermesRequest,
  HermesResponse,
  HistoryEntry,
  ImportKind,
  ImportPreview,
  ImportResult,
  SendRequestInput,
  TreeNode,
  WorkspaceConfig
} from "./types";

const hasTauri = "__TAURI_INTERNALS__" in window;

export const emptyRequest = (): HermesRequest => ({
  id: crypto.randomUUID(),
  name: "Untitled request",
  method: "GET",
  url: "https://api.example.com",
  params: [],
  headers: [],
  body: { kind: "none" },
  auth: { kind: "none" }
});

const demoWorkspace: WorkspaceConfig = {
  id: "demo-workspace",
  name: "Hermes Demo",
  version: 1
};

const demoRequest: HermesRequest = {
  id: "req-demo",
  name: "Example JSON",
  method: "GET",
  url: "https://api.example.com/users",
  params: [{ id: "p1", key: "limit", value: "10", enabled: true }],
  headers: [{ id: "h1", key: "Accept", value: "application/json", enabled: true }],
  body: { kind: "none" },
  auth: { kind: "none" }
};

async function command<T>(name: string, args?: Record<string, unknown>, fallback?: () => T): Promise<T> {
  if (!hasTauri) {
    if (!fallback) {
      throw new Error(`${name} requires the Tauri runtime`);
    }
    await new Promise((resolve) => window.setTimeout(resolve, 120));
    return fallback();
  }
  return invoke<T>(name, args);
}

export const api = {
  createWorkspace: (path: string, name: string) =>
    command<WorkspaceConfig>("create_workspace", { path, name }, () => demoWorkspace),
  openWorkspace: (path: string) => command<WorkspaceConfig>("open_workspace", { path }, () => demoWorkspace),
  readWorkspaceConfig: (workspace_path: string) =>
    command<WorkspaceConfig>("read_workspace_config", { workspacePath: workspace_path }, () => demoWorkspace),
  listWorkspaceTree: (workspace_path: string) =>
    command<TreeNode[]>("list_workspace_tree", { workspacePath: workspace_path }, () => [
      { kind: "request", id: demoRequest.id, name: demoRequest.name, path: "collections/example-json.yaml", method: "GET" }
    ]),
  readRequest: (workspace_path: string, request_path: string) =>
    command<HermesRequest>("read_request", { workspacePath: workspace_path, requestPath: request_path }, () => demoRequest),
  writeRequest: (workspace_path: string, request_path: string, request: HermesRequest) =>
    command<string>("write_request", { workspacePath: workspace_path, requestPath: request_path, request }, () => request_path),
  deleteRequest: (workspace_path: string, request_path: string) =>
    command<void>("delete_request", { workspacePath: workspace_path, requestPath: request_path }, () => undefined),
  duplicateRequest: (workspace_path: string, request_path: string) =>
    command<string>("duplicate_request", { workspacePath: workspace_path, requestPath: request_path }, () => "collections/example-json-copy.yaml"),
  createGroup: (workspace_path: string, group_path: string) =>
    command<string>("create_group", { workspacePath: workspace_path, groupPath: group_path }, () => `collections/${group_path}`),
  deleteGroup: (workspace_path: string, group_path: string, recursive = false) =>
    command<void>("delete_group", { workspacePath: workspace_path, groupPath: group_path, recursive }, () => undefined),
  renameGroup: (workspace_path: string, group_path: string, new_group_path: string) =>
    command<string>("rename_group", { workspacePath: workspace_path, groupPath: group_path, newGroupPath: new_group_path }, () => `collections/${new_group_path}`),
  sendRequest: (input: SendRequestInput) =>
    command<HermesResponse>("send_request", { input }, () => ({
      status: 200,
      status_text: "OK",
      headers: [{ id: "content-type", key: "content-type", value: "application/json", enabled: true }],
      body: JSON.stringify({ message: "Demo response from Hermes", request: input.request.url }, null, 2),
      content_type: "application/json",
      elapsed_ms: 42,
      size_bytes: 72
    })),
  listEnvironments: (workspace_path: string) =>
    command<HermesEnvironment[]>("list_environments", { workspacePath: workspace_path }, () => [
      {
        id: "local",
        name: "Local",
        values: [
          { id: "baseUrl", key: "baseUrl", value: "https://api.example.com", enabled: true },
          { id: "token", key: "token", value: "Stored in keychain", enabled: true, secret: true }
        ]
      }
    ]),
  readEnvironment: (workspace_path: string, environment_id: string) =>
    command<HermesEnvironment>("read_environment", { workspacePath: workspace_path, environmentId: environment_id }, () => ({
      id: environment_id,
      name: "Local",
      values: []
    })),
  writeEnvironment: (workspace_path: string, environment: HermesEnvironment) =>
    command<string>("write_environment", { workspacePath: workspace_path, environment }, () => environment.id),
  resolveVariables: (workspace_path: string, text: string, environment_id?: string) =>
    command<string>("resolve_variables", { workspacePath: workspace_path, text, environmentId: environment_id }, () => text.replace(/\{\{baseUrl\}\}/g, "https://api.example.com")),
  setSecret: (workspace_path: string, environment_id: string, key: string, value: string) =>
    command<void>("set_secret", { workspacePath: workspace_path, environmentId: environment_id, key, value }, () => undefined),
  getSecretMetadata: (workspace_path: string, environment_id: string) =>
    command<string[]>("get_secret_metadata", { workspacePath: workspace_path, environmentId: environment_id }, () => ["token"]),
  deleteSecret: (workspace_path: string, environment_id: string, key: string) =>
    command<void>("delete_secret", { workspacePath: workspace_path, environmentId: environment_id, key }, () => undefined),
  importPreview: (kind: ImportKind, payload: string) =>
    command<ImportPreview>("import_preview", { kind, payload }, () => ({ requests: [demoRequest], warnings: [] })),
  importCollection: (workspace_path: string, kind: ImportKind, payload: string) =>
    command<ImportResult>("import_collection", { workspacePath: workspace_path, kind, payload }, () => ({
      written: ["collections/imported/example-json.yaml"],
      warnings: []
    })),
  importCurl: (workspace_path: string, command_text: string) =>
    command<ImportResult>("import_curl", { workspacePath: workspace_path, commandText: command_text }, () => ({ written: ["collections/imported/curl.yaml"], warnings: [] })),
  importPostmanCollection: (workspace_path: string, payload: string) =>
    command<ImportResult>("import_postman_collection", { workspacePath: workspace_path, payload }, () => ({
      written: ["collections/imported/postman.yaml"],
      warnings: []
    })),
  importOpenApi: (workspace_path: string, payload: string) =>
    command<ImportResult>("import_openapi", { workspacePath: workspace_path, payload }, () => ({ written: ["collections/imported/openapi.yaml"], warnings: [] })),
  listHistory: (workspace_path: string) => command<HistoryEntry[]>("list_history", { workspacePath: workspace_path }, () => []),
  readHistoryEntry: (workspace_path: string, id: string) =>
    command<HistoryEntry>("read_history_entry", { workspacePath: workspace_path, id }, () => {
      throw new Error("History entry not found");
    }),
  clearHistory: (workspace_path: string) => command<void>("clear_history", { workspacePath: workspace_path }, () => undefined)
};

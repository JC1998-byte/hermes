export type HttpMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "PATCH"
  | "DELETE"
  | "HEAD"
  | "OPTIONS";

export interface KeyValueRow {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
  secret?: boolean;
}

export type RequestBody =
  | { kind: "none"; content?: string }
  | { kind: "json"; content: string }
  | { kind: "text"; content: string }
  | { kind: "form"; content: string }
  | { kind: "binary"; content: string };

export type AuthConfig =
  | { kind: "none" }
  | { kind: "basic"; username: string; password: string; password_secret?: string }
  | { kind: "bearer"; token: string; token_secret?: string }
  | { kind: "api_key"; placement: "header" | "query"; name: string; value: string; value_secret?: string }
  | {
      kind: "oauth2_client_credentials";
      token_url: string;
      client_id: string;
      client_secret: string;
      client_secret_ref?: string;
      scopes: string[];
    };

export interface HermesRequest {
  id: string;
  name: string;
  method: HttpMethod;
  url: string;
  params: KeyValueRow[];
  headers: KeyValueRow[];
  body: RequestBody;
  auth: AuthConfig;
  description?: string;
}

export interface HermesEnvironment {
  id: string;
  name: string;
  values: KeyValueRow[];
}

export interface WorkspaceConfig {
  id: string;
  name: string;
  version: number;
}

export type TreeNode =
  | { kind: "folder"; name: string; path: string; children: TreeNode[] }
  | { kind: "request"; name: string; path: string; method: HttpMethod; id: string };

export interface SendRequestInput {
  workspace_path: string;
  request: HermesRequest;
  environment_id?: string;
}

export interface HermesResponse {
  status: number;
  status_text: string;
  headers: KeyValueRow[];
  body: string;
  content_type?: string;
  elapsed_ms: number;
  size_bytes: number;
}

export interface HistoryEntry {
  id: string;
  workspace_id: string;
  request_name: string;
  method: HttpMethod;
  url: string;
  status?: number;
  elapsed_ms?: number;
  created_at: string;
  request: HermesRequest;
  response?: HermesResponse;
}

export interface ImportPreview {
  requests: HermesRequest[];
  warnings: string[];
}

export interface ImportResult {
  written: string[];
  warnings: string[];
}

export interface ApiError {
  code: string;
  message: string;
  detail?: string;
}

export type ImportKind = "curl" | "postman" | "openapi";

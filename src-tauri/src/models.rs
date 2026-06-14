use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_reqwest(&self) -> reqwest::Method {
        match self {
            Self::Get => reqwest::Method::GET,
            Self::Post => reqwest::Method::POST,
            Self::Put => reqwest::Method::PUT,
            Self::Patch => reqwest::Method::PATCH,
            Self::Delete => reqwest::Method::DELETE,
            Self::Head => reqwest::Method::HEAD,
            Self::Options => reqwest::Method::OPTIONS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValueRow {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub secret: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestBody {
    None { content: Option<String> },
    Json { content: String },
    Text { content: String },
    Form { content: String },
    Binary { content: String },
}

impl Default for RequestBody {
    fn default() -> Self {
        Self::None { content: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthConfig {
    None,
    Basic {
        username: String,
        password: String,
        #[serde(default)]
        password_secret: Option<String>,
    },
    Bearer {
        token: String,
        #[serde(default)]
        token_secret: Option<String>,
    },
    ApiKey {
        placement: ApiKeyPlacement,
        name: String,
        value: String,
        #[serde(default)]
        value_secret: Option<String>,
    },
    Oauth2ClientCredentials {
        token_url: String,
        client_id: String,
        client_secret: String,
        #[serde(default)]
        client_secret_ref: Option<String>,
        #[serde(default)]
        scopes: Vec<String>,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyPlacement {
    Header,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesRequest {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub params: Vec<KeyValueRow>,
    #[serde(default)]
    pub headers: Vec<KeyValueRow>,
    #[serde(default)]
    pub body: RequestBody,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesEnvironment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub values: Vec<KeyValueRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TreeNode {
    Folder {
        name: String,
        path: String,
        children: Vec<TreeNode>,
    },
    Request {
        name: String,
        path: String,
        method: HttpMethod,
        id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRequestInput {
    pub workspace_path: String,
    pub request: HermesRequest,
    #[serde(default)]
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<KeyValueRow>,
    pub body: String,
    #[serde(default)]
    pub content_type: Option<String>,
    pub elapsed_ms: u128,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub workspace_id: String,
    pub request_name: String,
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub elapsed_ms: Option<u128>,
    pub created_at: DateTime<Utc>,
    pub request: HermesRequest,
    #[serde(default)]
    pub response: Option<HermesResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub requests: Vec<HermesRequest>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub written: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    #[serde(default)]
    pub keys: Vec<String>,
}

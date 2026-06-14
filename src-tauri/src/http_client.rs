use std::{fs, time::Instant};

use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    history,
    models::{
        ApiKeyPlacement, AuthConfig, HermesResponse, HistoryEntry, KeyValueRow, RequestBody,
        SendRequestInput,
    },
    variables, workspace,
};

pub async fn send_request(input: SendRequestInput) -> Result<HermesResponse> {
    let workspace = workspace::read_workspace_config(input.workspace_path.clone())?;
    let env_id = input.environment_id.as_deref();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let mut request = input.request.clone();
    let resolved_url = variables::resolve_text(&input.workspace_path, env_id, &request.url)?;
    let mut url = Url::parse(&resolved_url)?;

    for row in request
        .params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        let key = variables::resolve_text(&input.workspace_path, env_id, &row.key)?;
        let value = variables::resolve_text(&input.workspace_path, env_id, &row.value)?;
        url.query_pairs_mut().append_pair(&key, &value);
    }

    if let AuthConfig::ApiKey {
        placement: ApiKeyPlacement::Query,
        name,
        value,
        ..
    } = &request.auth
    {
        let name = variables::resolve_text(&input.workspace_path, env_id, name)?;
        let value = variables::resolve_text(&input.workspace_path, env_id, value)?;
        url.query_pairs_mut().append_pair(&name, &value);
    }

    let mut builder = client.request(request.method.as_reqwest(), url);
    let mut headers = Vec::new();
    for row in request
        .headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        let key = variables::resolve_text(&input.workspace_path, env_id, &row.key)?;
        let value = variables::resolve_text(&input.workspace_path, env_id, &row.value)?;
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
            AppError::with_detail(
                "invalid_header",
                format!("Invalid header name '{key}'."),
                err.to_string(),
            )
        })?;
        let header_value = HeaderValue::from_str(&value).map_err(|err| {
            AppError::with_detail(
                "invalid_header",
                format!("Invalid value for header '{key}'."),
                err.to_string(),
            )
        })?;
        builder = builder.header(name, header_value);
        headers.push((key, value));
    }

    builder = apply_auth(
        builder,
        &client,
        &input.workspace_path,
        env_id,
        &request.auth,
    )
    .await?;
    builder = apply_body(
        builder,
        &input.workspace_path,
        env_id,
        &request.body,
        &headers,
    )?;

    let started = Instant::now();
    let response = builder.send().await?;
    let elapsed_ms = started.elapsed().as_millis();
    let status = response.status();
    let response_headers = response
        .headers()
        .iter()
        .map(|(key, value)| KeyValueRow {
            id: key.as_str().to_string(),
            key: key.as_str().to_string(),
            value: value.to_str().unwrap_or("<non-utf8>").to_string(),
            enabled: true,
            secret: false,
        })
        .collect::<Vec<_>>();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response.bytes().await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    let hermes_response = HermesResponse {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers: response_headers,
        body,
        content_type,
        elapsed_ms,
        size_bytes: bytes.len(),
    };

    request.url = resolved_url;
    let history_entry = HistoryEntry {
        id: Uuid::new_v4().to_string(),
        workspace_id: workspace.id,
        request_name: request.name.clone(),
        method: request.method.clone(),
        url: request.url.clone(),
        status: Some(hermes_response.status),
        elapsed_ms: Some(hermes_response.elapsed_ms),
        created_at: Utc::now(),
        request,
        response: Some(hermes_response.clone()),
    };
    history::append_history(&input.workspace_path, &history_entry)?;

    Ok(hermes_response)
}

async fn apply_auth(
    mut builder: reqwest::RequestBuilder,
    client: &reqwest::Client,
    workspace_path: &str,
    environment_id: Option<&str>,
    auth: &AuthConfig,
) -> Result<reqwest::RequestBuilder> {
    match auth {
        AuthConfig::None => {}
        AuthConfig::Basic {
            username, password, ..
        } => {
            let username = variables::resolve_text(workspace_path, environment_id, username)?;
            let password = variables::resolve_text(workspace_path, environment_id, password)?;
            builder = builder.basic_auth(username, Some(password));
        }
        AuthConfig::Bearer { token, .. } => {
            let token = variables::resolve_text(workspace_path, environment_id, token)?;
            builder = builder.bearer_auth(token);
        }
        AuthConfig::ApiKey {
            placement,
            name,
            value,
            ..
        } => {
            let name = variables::resolve_text(workspace_path, environment_id, name)?;
            let value = variables::resolve_text(workspace_path, environment_id, value)?;
            match placement {
                ApiKeyPlacement::Header => {
                    builder = builder.header(name, value);
                }
                ApiKeyPlacement::Query => {}
            }
        }
        AuthConfig::Oauth2ClientCredentials {
            token_url,
            client_id,
            client_secret,
            scopes,
            ..
        } => {
            let token_url = variables::resolve_text(workspace_path, environment_id, token_url)?;
            let client_id = variables::resolve_text(workspace_path, environment_id, client_id)?;
            let client_secret =
                variables::resolve_text(workspace_path, environment_id, client_secret)?;
            let scope = scopes.join(" ");
            let token = client
                .post(token_url)
                .form(&[
                    ("grant_type", "client_credentials"),
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("scope", scope.as_str()),
                ])
                .send()
                .await?
                .error_for_status()?
                .json::<OAuthTokenResponse>()
                .await?;
            builder = builder.bearer_auth(token.access_token);
        }
    }
    Ok(builder)
}

fn apply_body(
    mut builder: reqwest::RequestBuilder,
    workspace_path: &str,
    environment_id: Option<&str>,
    body: &RequestBody,
    headers: &[(String, String)],
) -> Result<reqwest::RequestBuilder> {
    match body {
        RequestBody::None { .. } => {}
        RequestBody::Json { content } => {
            if !has_header(headers, "content-type") {
                builder = builder.header(CONTENT_TYPE, "application/json");
            }
            builder = builder.body(variables::resolve_text(
                workspace_path,
                environment_id,
                content,
            )?);
        }
        RequestBody::Text { content } => {
            builder = builder.body(variables::resolve_text(
                workspace_path,
                environment_id,
                content,
            )?);
        }
        RequestBody::Form { content } => {
            if !has_header(headers, "content-type") {
                builder = builder.header(CONTENT_TYPE, "application/x-www-form-urlencoded");
            }
            builder = builder.body(variables::resolve_text(
                workspace_path,
                environment_id,
                content,
            )?);
        }
        RequestBody::Binary { content } => {
            let path = variables::resolve_text(workspace_path, environment_id, content)?;
            builder = builder.body(fs::read(path)?);
        }
    }
    Ok(builder)
}

fn has_header(headers: &[(String, String)], key: &str) -> bool {
    headers
        .iter()
        .any(|(header, _)| header.eq_ignore_ascii_case(key))
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;

    use super::*;
    use crate::models::{HermesRequest, HttpMethod};

    #[tokio::test]
    async fn sends_basic_get_request() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/ping").query_param("limit", "1");
            then.status(200)
                .header("content-type", "application/json")
                .body("{\"ok\":true}");
        });
        let dir = tempfile::tempdir().unwrap();
        workspace::create_workspace(dir.path().to_string_lossy().to_string(), "Test".to_string())
            .unwrap();
        let input = SendRequestInput {
            workspace_path: dir.path().to_string_lossy().to_string(),
            environment_id: Some("local".to_string()),
            request: HermesRequest {
                id: "req".to_string(),
                name: "Ping".to_string(),
                method: HttpMethod::Get,
                url: format!("{}/ping", server.base_url()),
                params: vec![KeyValueRow {
                    id: "limit".to_string(),
                    key: "limit".to_string(),
                    value: "1".to_string(),
                    enabled: true,
                    secret: false,
                }],
                headers: Vec::new(),
                body: RequestBody::None { content: None },
                auth: AuthConfig::None,
                description: None,
            },
        };
        let response = send_request(input).await.unwrap();
        assert_eq!(response.status, 200);
        assert!(response.body.contains("\"ok\":true"));
        mock.assert();
    }
}

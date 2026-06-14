use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{
        AuthConfig, HermesRequest, HttpMethod, ImportPreview, ImportResult, KeyValueRow,
        RequestBody,
    },
    workspace,
};

pub fn preview(kind: &str, payload: &str) -> Result<ImportPreview> {
    match kind {
        "curl" => Ok(ImportPreview {
            requests: vec![parse_curl(payload)?],
            warnings: Vec::new(),
        }),
        "postman" => parse_postman(payload),
        "openapi" => parse_openapi(payload),
        other => Err(AppError::user(
            "unsupported_import",
            format!("Unsupported import kind '{other}'."),
        )),
    }
}

pub fn import_collection(workspace_path: &str, kind: &str, payload: &str) -> Result<ImportResult> {
    let preview = preview(kind, payload)?;
    let mut written = Vec::new();
    for request in preview.requests {
        let path = format!(
            "collections/imported/{}-{}.yaml",
            slugify(&request.name),
            Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );
        written.push(workspace::write_request(
            workspace_path.to_string(),
            path,
            request,
        )?);
    }
    Ok(ImportResult {
        written,
        warnings: preview.warnings,
    })
}

fn parse_curl(command: &str) -> Result<HermesRequest> {
    let tokens = shell_words::split(command).map_err(|err| {
        AppError::with_detail(
            "invalid_curl",
            "Could not parse cURL command.",
            err.to_string(),
        )
    })?;
    let mut method: Option<HttpMethod> = None;
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let mut index = 0;

    while index < tokens.len() {
        match tokens[index].as_str() {
            "curl" => {}
            "-X" | "--request" => {
                index += 1;
                method = tokens.get(index).and_then(|value| method_from_str(value));
            }
            "-H" | "--header" => {
                index += 1;
                if let Some(header) = tokens.get(index) {
                    if let Some((key, value)) = header.split_once(':') {
                        headers.push(KeyValueRow {
                            id: Uuid::new_v4().to_string(),
                            key: key.trim().to_string(),
                            value: value.trim().to_string(),
                            enabled: true,
                            secret: false,
                        });
                    }
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                index += 1;
                body = tokens.get(index).cloned();
                method.get_or_insert(HttpMethod::Post);
            }
            token if token.starts_with("http://") || token.starts_with("https://") => {
                url = token.to_string();
            }
            _ => {}
        }
        index += 1;
    }

    if url.is_empty() {
        return Err(AppError::user(
            "invalid_curl",
            "The cURL command does not contain an HTTP URL.",
        ));
    }

    let name = url
        .split('/')
        .filter(|part| !part.is_empty())
        .last()
        .unwrap_or("Imported cURL")
        .to_string();

    Ok(HermesRequest {
        id: Uuid::new_v4().to_string(),
        name,
        method: method.unwrap_or(HttpMethod::Get),
        url,
        params: Vec::new(),
        headers,
        body: body
            .map(|content| {
                if looks_like_json(&content) {
                    RequestBody::Json { content }
                } else {
                    RequestBody::Text { content }
                }
            })
            .unwrap_or_default(),
        auth: AuthConfig::None,
        description: None,
    })
}

fn parse_postman(payload: &str) -> Result<ImportPreview> {
    let value: JsonValue = serde_json::from_str(payload)?;
    let mut requests = Vec::new();
    let mut warnings = Vec::new();
    if let Some(items) = value.get("item").and_then(|item| item.as_array()) {
        collect_postman_items(items, &mut requests, &mut warnings)?;
    }
    Ok(ImportPreview { requests, warnings })
}

fn collect_postman_items(
    items: &[JsonValue],
    requests: &mut Vec<HermesRequest>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    for item in items {
        if let Some(children) = item.get("item").and_then(|value| value.as_array()) {
            collect_postman_items(children, requests, warnings)?;
            continue;
        }

        let request_value = match item.get("request") {
            Some(value) => value,
            None => continue,
        };
        let name = item
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("Imported request");
        let method = request_value
            .get("method")
            .and_then(|value| value.as_str())
            .and_then(method_from_str)
            .unwrap_or(HttpMethod::Get);
        let url = postman_url(request_value.get("url")).unwrap_or_default();
        if url.is_empty() {
            warnings.push(format!("Skipped '{name}' because it has no URL."));
            continue;
        }
        let headers = request_value
            .get("header")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|header| {
                Some(KeyValueRow {
                    id: Uuid::new_v4().to_string(),
                    key: header.get("key")?.as_str()?.to_string(),
                    value: header
                        .get("value")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                    enabled: !header
                        .get("disabled")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    secret: false,
                })
            })
            .collect();
        let body = request_value
            .get("body")
            .and_then(|body| body.get("raw"))
            .and_then(|value| value.as_str())
            .map(|content| {
                if looks_like_json(content) {
                    RequestBody::Json {
                        content: content.to_string(),
                    }
                } else {
                    RequestBody::Text {
                        content: content.to_string(),
                    }
                }
            })
            .unwrap_or_default();

        requests.push(HermesRequest {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            method,
            url,
            params: Vec::new(),
            headers,
            body,
            auth: AuthConfig::None,
            description: None,
        });
    }
    Ok(())
}

fn parse_openapi(payload: &str) -> Result<ImportPreview> {
    let value: YamlValue = serde_yaml::from_str(payload)?;
    let mut requests = Vec::new();
    let mut warnings = Vec::new();
    let base_url = first_server_url(&value).unwrap_or_else(|| "{{baseUrl}}".to_string());
    let Some(paths) = get_key(&value, "paths").and_then(|value| value.as_mapping()) else {
        return Err(AppError::user(
            "invalid_openapi",
            "OpenAPI document does not contain a paths object.",
        ));
    };

    for (path_key, path_value) in paths {
        let Some(path_template) = path_key.as_str() else {
            continue;
        };
        let Some(operations) = path_value.as_mapping() else {
            continue;
        };
        for (method_key, operation) in operations {
            let Some(method_name) = method_key.as_str() else {
                continue;
            };
            let Some(method) = method_from_str(method_name) else {
                continue;
            };
            let name = get_key(operation, "operationId")
                .and_then(|value| value.as_str())
                .or_else(|| get_key(operation, "summary").and_then(|value| value.as_str()))
                .unwrap_or(path_template);
            let description = get_key(operation, "description")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            let (params, headers) = openapi_parameters(operation);
            let body = openapi_body(operation);

            requests.push(HermesRequest {
                id: Uuid::new_v4().to_string(),
                name: name.to_string(),
                method,
                url: format!("{}{}", base_url.trim_end_matches('/'), path_template),
                params,
                headers,
                body,
                auth: AuthConfig::None,
                description,
            });
        }
    }

    if requests.is_empty() {
        warnings
            .push("No supported HTTP operations were found in the OpenAPI document.".to_string());
    }

    Ok(ImportPreview { requests, warnings })
}

fn openapi_parameters(operation: &YamlValue) -> (Vec<KeyValueRow>, Vec<KeyValueRow>) {
    let mut params = Vec::new();
    let mut headers = Vec::new();
    if let Some(parameters) = get_key(operation, "parameters").and_then(|value| value.as_sequence())
    {
        for parameter in parameters {
            let name = get_key(parameter, "name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let location = get_key(parameter, "in")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let row = KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key: name.to_string(),
                value: String::new(),
                enabled: false,
                secret: false,
            };
            match location {
                "query" => params.push(row),
                "header" => headers.push(row),
                _ => {}
            }
        }
    }
    (params, headers)
}

fn openapi_body(operation: &YamlValue) -> RequestBody {
    let Some(request_body) = get_key(operation, "requestBody") else {
        return RequestBody::default();
    };
    let Some(content) = get_key(request_body, "content").and_then(|value| value.as_mapping())
    else {
        return RequestBody::default();
    };
    let json_key = YamlValue::String("application/json".to_string());
    if content.contains_key(&json_key) {
        return RequestBody::Json {
            content: "{\n  \n}".to_string(),
        };
    }
    RequestBody::Text {
        content: String::new(),
    }
}

fn postman_url(value: Option<&JsonValue>) -> Option<String> {
    match value? {
        JsonValue::String(url) => Some(url.clone()),
        JsonValue::Object(map) => {
            if let Some(raw) = map.get("raw").and_then(|value| value.as_str()) {
                return Some(raw.to_string());
            }
            let protocol = map
                .get("protocol")
                .and_then(|value| value.as_str())
                .unwrap_or("https");
            let host = map
                .get("host")
                .and_then(|value| value.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join(".")
                })
                .or_else(|| {
                    map.get("host")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned)
                })?;
            let path = map
                .get("path")
                .and_then(|value| value.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default();
            Some(format!(
                "{protocol}://{host}/{}",
                path.trim_start_matches('/')
            ))
        }
        _ => None,
    }
}

fn first_server_url(value: &YamlValue) -> Option<String> {
    get_key(value, "servers")
        .and_then(|servers| servers.as_sequence())
        .and_then(|servers| servers.first())
        .and_then(|server| get_key(server, "url"))
        .and_then(|url| url.as_str())
        .map(ToOwned::to_owned)
}

fn get_key<'a>(value: &'a YamlValue, key: &str) -> Option<&'a YamlValue> {
    value.as_mapping()?.get(&YamlValue::String(key.to_string()))
}

fn method_from_str(value: &str) -> Option<HttpMethod> {
    match value.to_ascii_uppercase().as_str() {
        "GET" => Some(HttpMethod::Get),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        "PATCH" => Some(HttpMethod::Patch),
        "DELETE" => Some(HttpMethod::Delete),
        "HEAD" => Some(HttpMethod::Head),
        "OPTIONS" => Some(HttpMethod::Options),
        _ => None,
    }
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

fn slugify(value: &str) -> String {
    let slug = value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "request".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_curl_with_header_and_body() {
        let request = parse_curl("curl -X POST https://example.com/users -H 'content-type: application/json' -d '{\"name\":\"Ada\"}'").unwrap();
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.headers[0].key, "content-type");
        assert!(matches!(request.body, RequestBody::Json { .. }));
    }

    #[test]
    fn parses_openapi_paths() {
        let input = r#"
openapi: 3.0.0
servers:
  - url: https://api.example.com
paths:
  /users:
    get:
      operationId: listUsers
      parameters:
        - name: limit
          in: query
"#;
        let preview = parse_openapi(input).unwrap();
        assert_eq!(preview.requests.len(), 1);
        assert_eq!(preview.requests[0].name, "listUsers");
        assert_eq!(preview.requests[0].params[0].key, "limit");
    }

    #[test]
    fn parses_postman_items() {
        let input = r#"{"item":[{"name":"Ping","request":{"method":"GET","url":"https://example.com/ping"}}]}"#;
        let preview = parse_postman(input).unwrap();
        assert_eq!(preview.requests.len(), 1);
        assert_eq!(preview.requests[0].name, "Ping");
    }
}

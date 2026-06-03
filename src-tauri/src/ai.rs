use crate::config::AppConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const PREAMBLE: &str = "You are a writing assistant that improves the user's writing. Always respond in the same language as the input. Preserve the user's voice and intent. Return only the improved text, nothing else.";

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

enum Auth {
    ApiKey(String),
    Bearer(String),
}

async fn resolve_auth(config: &AppConfig) -> Result<Auth, String> {
    if config.auth_mode == "api_key" {
        if config.api_key.is_empty() {
            return Err("API key not set. Open Fix My Wording settings to configure.".to_string());
        }
        Ok(Auth::ApiKey(config.api_key.clone()))
    } else {
        Ok(Auth::Bearer(crate::auth::get_access_token().await?))
    }
}

pub async fn improve_text(text: &str, config: &AppConfig) -> Result<String, String> {
    let system = format!(
        "{PREAMBLE}\n\n{}\n\nIMPORTANT: You MUST wrap your entire response in <result></result> XML tags. Output NOTHING outside these tags.",
        config.system_prompt
    );

    let client = Client::new();
    let body = AnthropicRequest {
        model: config.model.clone(),
        max_tokens: 4096,
        system,
        messages: vec![Message {
            role: "user".to_string(),
            content: text.to_string(),
        }],
    };

    let mut auth = resolve_auth(config).await?;
    let mut resp = send_request(&client, &body, &auth).await?;

    // A 401 in OAuth mode usually means the cached token was revoked early —
    // drop it and retry once with a freshly minted one.
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED && matches!(auth, Auth::Bearer(_)) {
        crate::auth::invalidate_token();
        auth = resolve_auth(config).await?;
        resp = send_request(&client, &body, &auth).await?;
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API error {status}: {body}"));
    }

    let parsed: AnthropicResponse = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;

    let raw = parsed
        .content
        .first()
        .and_then(|b| b.text.clone())
        .ok_or_else(|| "Empty response from API".to_string())?;

    extract_result(&raw).ok_or_else(|| format!("No <result> tag found in response: {raw}"))
}

async fn send_request(
    client: &Client,
    body: &AnthropicRequest,
    auth: &Auth,
) -> Result<reqwest::Response, String> {
    let req = client
        .post("https://api.anthropic.com/v1/messages")
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(body);
    let req = match auth {
        Auth::ApiKey(key) => req.header("x-api-key", key),
        Auth::Bearer(token) => req.header("authorization", format!("Bearer {token}")),
    };
    req.send().await.map_err(|e| format!("Request failed: {e}"))
}

fn extract_result(text: &str) -> Option<String> {
    let start = text.find("<result>")?;
    let end = text.find("</result>")?;
    Some(text[start + "<result>".len()..end].trim().to_string())
}

use log::info;
use serde::Deserialize;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed locations only — no PATH lookup, so another process can't shadow the
/// binary by editing PATH.
const ANT_PATHS: &[&str] = &["/opt/homebrew/bin/ant", "/usr/local/bin/ant"];

/// Refresh the cached token this many seconds before it actually expires.
const EXPIRY_MARGIN_SECS: u64 = 120;

struct CachedToken {
    token: String,
    expires_at: u64,
}

static CACHE: Mutex<Option<CachedToken>> = Mutex::new(None);

#[derive(Deserialize)]
struct AntCredentials {
    access_token: String,
    expires_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn ant_binary() -> Result<&'static str, String> {
    ANT_PATHS
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .copied()
        .ok_or_else(|| {
            "Anthropic CLI not found. Install it (brew install ant) and run `ant auth login`."
                .to_string()
        })
}

/// Runs `ant auth print-credentials`, which transparently refreshes the access
/// token via the stored refresh token when it is expired or near expiry.
fn fetch_token() -> Result<CachedToken, String> {
    let bin = ant_binary()?;
    let output = Command::new(bin)
        .args(["auth", "print-credentials"])
        .output()
        .map_err(|e| format!("Failed to run ant CLI: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Not logged in to Anthropic. Run `ant auth login` in a terminal. ({})",
            stderr.trim()
        ));
    }

    let creds: AntCredentials = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Unexpected ant CLI output: {e}"))?;

    info!(
        "Fetched OAuth token from ant CLI, valid for {}s",
        creds.expires_at.saturating_sub(now_secs())
    );

    Ok(CachedToken {
        token: creds.access_token,
        expires_at: creds.expires_at,
    })
}

/// Returns a valid OAuth access token, using the in-memory cache when fresh.
pub async fn get_access_token() -> Result<String, String> {
    {
        let cache = CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = cache.as_ref() {
            if cached.expires_at > now_secs() + EXPIRY_MARGIN_SECS {
                return Ok(cached.token.clone());
            }
        }
    }

    let fresh = tauri::async_runtime::spawn_blocking(fetch_token)
        .await
        .map_err(|e| format!("Token task failed: {e}"))??;

    let token = fresh.token.clone();
    *CACHE.lock().map_err(|e| e.to_string())? = Some(fresh);
    Ok(token)
}

/// Drops the cached token (e.g. after a 401) so the next call re-fetches.
pub fn invalidate_token() {
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
}

/// Returns the logged-in account email, or an error if not authenticated.
/// Used by the settings UI to show login status.
pub fn auth_status() -> Result<String, String> {
    #[derive(Deserialize)]
    struct StatusCreds {
        account_email: Option<String>,
    }

    let bin = ant_binary()?;
    let output = Command::new(bin)
        .args(["auth", "print-credentials"])
        .output()
        .map_err(|e| format!("Failed to run ant CLI: {e}"))?;

    if !output.status.success() {
        return Err("Not logged in. Run `ant auth login` in a terminal.".to_string());
    }

    let creds: StatusCreds = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Unexpected ant CLI output: {e}"))?;
    Ok(creds.account_email.unwrap_or_else(|| "logged in".to_string()))
}

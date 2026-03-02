use crate::{ai, config};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use log::{debug, error, info};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Wrapper for Enigo stored in Tauri's managed state.
/// Uses Option so we can retry initialization if permissions weren't granted at startup.
pub struct EnigoState(pub Mutex<Option<Enigo>>);

impl EnigoState {
    pub fn new() -> Self {
        let enigo = Enigo::new(&Settings::default()).ok();
        if enigo.is_some() {
            info!("Enigo initialized OK");
        } else {
            info!("Enigo not yet available (permissions may be missing, will retry)");
        }
        Self(Mutex::new(enigo))
    }

    /// Get or lazily initialize Enigo. Retries if previous init failed.
    pub fn get_or_init(&self) -> Result<std::sync::MutexGuard<'_, Option<Enigo>>, String> {
        let mut guard = self.0.lock().map_err(|e| format!("Enigo lock failed: {e}"))?;
        if guard.is_none() {
            info!("Retrying Enigo initialization...");
            match Enigo::new(&Settings::default()) {
                Ok(enigo) => {
                    info!("Enigo initialized OK (retry succeeded)");
                    *guard = Some(enigo);
                }
                Err(e) => {
                    return Err(format!("Enigo init failed (grant Accessibility permission and retry): {e}"));
                }
            }
        }
        Ok(guard)
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

static ICON_IDLE: &[u8] = include_bytes!("../icons/icon.png");
static ICON_DIM: &[u8] = include_bytes!("../icons/icon_processing.png");
static PULSING: AtomicBool = AtomicBool::new(false);

pub fn handle_hotkey(app: &tauri::AppHandle) {
    spawn_improve(app, Duration::ZERO);
}

/// Triggered from the tray menu — needs a short delay so the menu dismisses
/// and focus returns to the previous app before we simulate Cmd+C.
pub fn handle_hotkey_from_menu(app: &tauri::AppHandle) {
    spawn_improve(app, Duration::from_millis(200));
}

fn spawn_improve(app: &tauri::AppHandle, pre_delay: Duration) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if !pre_delay.is_zero() {
            tokio::time::sleep(pre_delay).await;
        }
        start_pulse(&app);
        let result = process_selection(&app).await;
        stop_pulse(&app);
        if let Err(e) = result {
            error!("hotkey error: {e}");
            #[cfg(debug_assertions)]
            {
                use tauri::Emitter;
                app.emit("fmw-error", &e).ok();
            }
        }
    });
}

fn start_pulse(app: &tauri::AppHandle) {
    PULSING.store(true, Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut bright = true;
        while PULSING.load(Ordering::SeqCst) {
            let icon_bytes: &[u8] = if bright { ICON_IDLE } else { ICON_DIM };
            let a1 = app.clone();
            let a2 = app.clone();
            let _ = a1.run_on_main_thread(move || {
                if let Some(tray) = a2.tray_by_id("fmw-tray") {
                    if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
                        tray.set_icon(Some(icon)).ok();
                    }
                }
            });
            bright = !bright;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

fn stop_pulse(app: &tauri::AppHandle) {
    PULSING.store(false, Ordering::SeqCst);
    let a1 = app.clone();
    let a2 = app.clone();
    let _ = a1.run_on_main_thread(move || {
        if let Some(tray) = a2.tray_by_id("fmw-tray") {
            if let Ok(icon) = tauri::image::Image::from_bytes(ICON_IDLE) {
                tray.set_icon(Some(icon)).ok();
            }
        }
    });
}

async fn process_selection(app: &tauri::AppHandle) -> Result<(), String> {
    info!("--- hotkey triggered ---");

    // Save current clipboard content
    let original_clipboard = app.clipboard().read_text().unwrap_or_default();
    debug!("original clipboard: {:?}", truncate(&original_clipboard, 100));

    // Simulate Cmd+C to copy selected text
    debug!("simulating Cmd+C...");
    simulate_copy(app)?;
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Read the copied text
    let selected_text = app
        .clipboard()
        .read_text()
        .map_err(|e| format!("Clipboard read failed: {e}"))?;
    debug!("selected text: {:?}", truncate(&selected_text, 200));

    if selected_text.is_empty() || selected_text == original_clipboard {
        // Nothing was selected or clipboard didn't change — restore and bail
        app.clipboard()
            .write_text(&original_clipboard)
            .map_err(|e| format!("Clipboard restore failed: {e}"))?;
        info!("no text selected (clipboard unchanged), aborting");
        return Err("No text selected".to_string());
    }

    // Call AI to improve
    let cfg = config::load_config();
    let key_preview = if cfg.api_key.is_empty() {
        "MISSING".to_string()
    } else {
        format!("{}...", &cfg.api_key[..8.min(cfg.api_key.len())])
    };
    debug!("config: model={}, api_key={}, prompt={:?}",
        cfg.model, key_preview, truncate(&cfg.system_prompt, 80)
    );
    info!("calling AI...");
    let improved = match ai::improve_text(&selected_text, &cfg).await {
        Ok(t) => t,
        Err(e) => {
            // Restore clipboard before bailing — otherwise the next hotkey press
            // sees original_clipboard == selected_text and wrongly reports "No text selected".
            app.clipboard().write_text(&original_clipboard).ok();
            return Err(e);
        }
    };
    debug!("AI response: {:?}", truncate(&improved, 200));

    // Save last result for "Copy Last Result" tray menu
    if let Some(state) = app.try_state::<crate::LastResult>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(improved.clone());
        }
    }

    // Write improved text to clipboard
    app.clipboard()
        .write_text(&improved)
        .map_err(|e| format!("Clipboard write failed: {e}"))?;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate Cmd+V to paste
    debug!("simulating Cmd+V...");
    simulate_paste(app)?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Restore original clipboard
    app.clipboard()
        .write_text(&original_clipboard)
        .map_err(|e| format!("Clipboard restore failed: {e}"))?;

    info!("--- done ---");
    Ok(())
}

// Platform-specific virtual keycodes (same as Handy)
#[cfg(target_os = "macos")]
const COPY_KEYS: (Key, Key) = (Key::Meta, Key::Other(8));   // Cmd + keycode 8 = C
#[cfg(target_os = "macos")]
const PASTE_KEYS: (Key, Key) = (Key::Meta, Key::Other(9));   // Cmd + keycode 9 = V

#[cfg(target_os = "windows")]
const COPY_KEYS: (Key, Key) = (Key::Control, Key::Other(0x43));  // Ctrl + VK_C
#[cfg(target_os = "windows")]
const PASTE_KEYS: (Key, Key) = (Key::Control, Key::Other(0x56)); // Ctrl + VK_V

#[cfg(target_os = "linux")]
const COPY_KEYS: (Key, Key) = (Key::Control, Key::Unicode('c'));
#[cfg(target_os = "linux")]
const PASTE_KEYS: (Key, Key) = (Key::Control, Key::Unicode('v'));

fn send_key_combo(app: &tauri::AppHandle, modifier: Key, key: Key) -> Result<(), String> {
    let enigo_state = app.try_state::<EnigoState>()
        .ok_or("EnigoState not managed")?;
    let mut guard = enigo_state.get_or_init()?;
    let enigo = guard.as_mut().ok_or("Enigo not available")?;

    enigo.key(modifier, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(key, Direction::Click).map_err(|e| e.to_string())?;

    std::thread::sleep(Duration::from_millis(100));

    enigo.key(modifier, Direction::Release).map_err(|e| e.to_string())?;

    Ok(())
}

fn simulate_copy(app: &tauri::AppHandle) -> Result<(), String> {
    send_key_combo(app, COPY_KEYS.0, COPY_KEYS.1)
}

fn simulate_paste(app: &tauri::AppHandle) -> Result<(), String> {
    send_key_combo(app, PASTE_KEYS.0, PASTE_KEYS.1)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

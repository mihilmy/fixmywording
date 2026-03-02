mod ai;
mod config;
mod hotkey;

use config::AppConfig;
use log::info;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_log::{Builder as LogBuilder, RotationStrategy, Target, TargetKind};

/// Stores the last improved text so the user can copy it from the tray menu.
pub struct LastResult(pub Mutex<Option<String>>);

#[tauri::command]
fn get_config() -> AppConfig {
    config::load_config()
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    config::save_config(&config)
}

#[tauri::command]
fn get_default_config() -> AppConfig {
    AppConfig::default()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            LogBuilder::new()
                .level(log::LevelFilter::Debug)
                .max_file_size(500_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("fixmywording".into()),
                    }),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![get_config, save_config, get_default_config])
        .setup(|app| {
            info!("=== Fix My Wording started ===");

            // Hide from dock — run as menu bar only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Initialize enigo state (will retry lazily if permissions aren't granted yet)
            app.manage(hotkey::EnigoState::new());

            // Initialize last result state
            app.manage(LastResult(Mutex::new(None)));

            // Build tray menu
            let fix_text_item = MenuItemBuilder::with_id("fix_text", "Fix Selected Text")
                .accelerator("CmdOrCtrl+Shift+K")
                .build(app)?;
            let copy_last_item = MenuItemBuilder::with_id("copy_last", "Copy Last Result").build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Fix My Wording").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&fix_text_item)
                .item(&copy_last_item)
                .separator()
                .item(&settings_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Build tray icon
            let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
            TrayIconBuilder::with_id("fmw-tray")
                .icon(icon)
                .icon_as_template(false)
                .menu(&menu)
                .tooltip("Fix My Wording — your words, but better")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "fix_text" => {
                        info!("Tray menu: Fix Selected Text");
                        hotkey::handle_hotkey_from_menu(app);
                    }
                    "copy_last" => {
                        if let Some(state) = app.try_state::<LastResult>() {
                            if let Ok(guard) = state.0.lock() {
                                if let Some(text) = guard.as_ref() {
                                    app.clipboard().write_text(text).ok();
                                }
                            }
                        }
                    }
                    "settings" => {
                        open_settings(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Cmd+Shift+K — improve selected text
            let shortcut_improve: Shortcut = "CommandOrControl+Shift+K".parse().unwrap();
            info!("Registering global shortcut: {:?}", shortcut_improve);
            let handle = app.handle().clone();
            app.global_shortcut().on_shortcut(shortcut_improve, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    info!("Shortcut pressed!");
                    hotkey::handle_hotkey(&handle);
                }
            })?;
            info!("Global shortcut registered OK");

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
                // Hide from dock when settings window is closed
                #[cfg(target_os = "macos")]
                window
                    .app_handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory)
                    .ok();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Fix My Wording");
}

fn open_settings(app: &tauri::AppHandle) {
    // Show in dock when settings window is open
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Regular).ok();

    if let Some(window) = app.get_webview_window("settings") {
        window.show().ok();
        window.set_focus().ok();
        return;
    }

    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("Fix My Wording Settings")
        .inner_size(600.0, 660.0)
        .resizable(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .ok();
}

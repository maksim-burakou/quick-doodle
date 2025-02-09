#[cfg_attr(mobile, tauri::mobile_entry_point)]
use tauri::{
    Emitter,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    Manager, AppHandle, Wry, Result
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use std::sync::{Arc, Mutex};

// Define the state
struct WindowState {
    is_visible: Mutex<bool>,
    tray_icon: Arc<Mutex<Option<TrayIcon>>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            is_visible: Mutex::new(false),
            tray_icon: Arc::new(Mutex::new(None)),
        }
    }
}

fn handle_event(app: &AppHandle, event_name: &str) {
  if let Some(window) = app.get_webview_window("main") {
      eprintln!("EVENT: {}", event_name);
      let _ = window.emit(event_name, {}).map_err(|e| {
          eprintln!("Error emitting {}: {:?}", event_name, e);
      });
  }
}

fn create_tray_menu(app: &AppHandle, visibility: bool) -> Result<Menu<Wry>> {
  let menu_item_color = MenuItem::with_id(app, "color", "Color", false, None::<&str>)?;
  let menu_item_undo = MenuItem::with_id(app, "undo", "Undo", visibility, Some("CmdOrCtrl+Z"))?;
  let menu_item_redo = MenuItem::with_id(app, "redo", "Redo", visibility, Some("Shift+CmdOrCtrl+Z"))?;
  let menu_item_clear = MenuItem::with_id(app, "clear", "Clear", visibility, Some("CmdOrCtrl+C"))?;
  let menu_item_reset = MenuItem::with_id(app, "reset", "Reset", visibility, Some("CmdOrCtrl+R"))?;
  let menu_item_quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
  let menu_item_quit_canvas = MenuItem::with_id(
      app,
      "quit_canvas",
      if visibility { "Quit Drawing Canvas" } else { "New Drawing Canvas" },
      true,
      Some("Shift+CmdOrCtrl+D"),
  )?;
  let menu_item_hide_canvas = MenuItem::with_id(
      app,
      "hide_canvas",
      if visibility { "Hide Drawing Canvas" } else { "Show Drawing Canvas" },
      true,
      Some("Shift+CmdOrCtrl+S"),
  )?;
  let menu_item_separator = PredefinedMenuItem::separator(app)?;
  let menu_item_shortcuts_config = MenuItem::with_id(app, "shortcuts", "Edit Shortcuts", false, None::<&str>)?;

  return Menu::with_items(
      app,
      &[
          &menu_item_color,
          &menu_item_separator,
          &menu_item_undo,
          &menu_item_redo,
          &menu_item_clear,
          &menu_item_reset,
          &menu_item_separator,
          &menu_item_hide_canvas,
          &menu_item_quit_canvas,
          &menu_item_separator,
          &menu_item_shortcuts_config,
          &menu_item_quit,
      ],
  )
}


fn toggle_window(app: &AppHandle) {
    let state = app.state::<WindowState>();
    let mut is_visible = state.is_visible.lock().unwrap();
    let tray_icon = state.tray_icon.lock().unwrap();

    if let Some(window) = app.get_webview_window("main") {
        let new_menu = create_tray_menu(app, !*is_visible).unwrap();
        if *is_visible {
            let _ = window.hide();
            if let Some(ref tray) = *tray_icon {
                let _ = tray.set_icon(Some(Image::from_path("./icons/tray/tray_icon--inactive.png").unwrap()));
                let _ = tray.set_icon_as_template(true);
                let _ = tray.set_menu(Some(new_menu));
            }
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            if let Some(ref tray) = *tray_icon {
                let _ = tray.set_icon(Some(Image::from_path("./icons/tray/tray_icon--active.png").unwrap()));
                let _ = tray.set_icon_as_template(false);
                let _ = tray.set_menu(Some(new_menu));
            }
        }
        *is_visible = !*is_visible;
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize state
            let state = WindowState::new();
            app.manage(state);

            // Hide dock icon on macOS
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Tray menu items
            let tray_menu = create_tray_menu(app.app_handle(), false)?;

            // Create tray icon
            let tray_icon = TrayIconBuilder::new()
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                  match event.id.as_ref() {
                      "quit" => app.exit(0),
                      "reset" => handle_event(app, "reset-canvas"),
                      "clear" => handle_event(app, "clear-canvas"),
                      "undo" => handle_event(app, "undo-canvas"),
                      "redo" => handle_event(app, "redo-canvas"),
                      "hide_canvas" => toggle_window(app),
                      "quit_canvas" => {
                          toggle_window(app);
                          handle_event(app, "reset-canvas");
                      },
                      &_ => {}
                  }
                })
                .icon(Image::from_path("./icons/tray/tray_icon--inactive.png")?)
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(&tray.app_handle());
                        handle_event(&tray.app_handle(), "reset-canvas");
                    }
                })
                .build(app)?;

            // Store tray icon in state correctly
            {
                let state = app.state::<WindowState>();
                let mut tray_lock = state.tray_icon.lock().unwrap();
                *tray_lock = Some(tray_icon);
            }

            // Define shortcuts
            #[cfg(target_os = "macos")]
            let ctrl_s_shortcut = Shortcut::new(Some(Modifiers::SHIFT | Modifiers::SUPER), Code::KeyS);
  
            #[cfg(not(target_os = "macos"))]
            let ctrl_s_shortcut = Shortcut::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyS);

            #[cfg(target_os = "macos")]
            let ctrl_d_shortcut = Shortcut::new(Some(Modifiers::SHIFT | Modifiers::SUPER), Code::KeyD);

            #[cfg(not(target_os = "macos"))]
            let ctrl_d_shortcut = Shortcut::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyD);

            let global_shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if shortcut == &ctrl_s_shortcut {
                            toggle_window(app);
                        } else if shortcut == &ctrl_d_shortcut {
                            handle_event(app, "reset-canvas");
                            toggle_window(app);
                        }
                    }
                })
                .build();

            app.handle().plugin(global_shortcut_plugin)?;
            app.global_shortcut().register(ctrl_s_shortcut)?;
            app.global_shortcut().register(ctrl_d_shortcut)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

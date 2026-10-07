mod platform;
mod storage;
mod timer;

use platform::{awake_ms, local_date};
use std::sync::Mutex;
use storage::Storage;
use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, AppHandle, Emitter, Manager, WebviewWindowBuilder};
use timer::{Phase, Settings, Snapshot, Status, Timer};

struct Core { timer: Timer, storage: Storage, last_save_ms: u64 }
struct Desktop {
    core: Mutex<Core>,
    toggle: MenuItem<tauri::Wry>,
    last_menu_text: Mutex<String>,
    last_published: Mutex<String>,
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show(); let _ = window.unminimize(); let _ = window.set_focus();
    }
}

fn completion_notice(app: &AppHandle, phase: Phase, sound: bool) {
    let body = if phase == Phase::Focus { "这一段专注完成了。准备好后手动开始休息。" } else { "休息结束了。准备好后手动开始下一段专注。" };
    if sound { platform::short_beep(); }
    // Use the native toast result: the generic plugin discards delivery errors on Windows.
    // A silent toast plus one independent beep also respects the sound toggle.
    let app_id = if cfg!(debug_assertions) { tauri_winrt_notification::Toast::POWERSHELL_APP_ID } else { &app.config().identifier };
    if let Err(error) = tauri_winrt_notification::Toast::new(app_id).title(&format!("{}完成", phase.label())).text1(body).sound(None).show() {
        if let Some(desktop) = app.try_state::<Desktop>() {
            if let Ok(mut core) = desktop.core.lock() {
                core.timer.warning = Some(format!("系统通知未能发送，窗口和托盘仍会显示完成状态：{error}"));
            }
        }
    }
}

fn publish(app: &AppHandle, snapshot: &Snapshot) {
    if let Some(desktop) = app.try_state::<Desktop>() {
        let mut comparable = snapshot.clone();
        comparable.revision = 0;
        if let Ok(key) = serde_json::to_string(&comparable) {
            if let Ok(mut previous) = desktop.last_published.lock() {
                if *previous == key { return; }
                *previous = key;
            }
        }
    }
    let _ = app.emit("timer-state", snapshot);
    if let Some(tray) = app.tray_by_id("pomodoro-tray") {
        let time = format!("{:02}:{:02}", snapshot.remaining_seconds / 60, snapshot.remaining_seconds % 60);
        let status = match snapshot.status { Status::Running => "计时中", Status::Paused => "已暂停", Status::Completed => "已完成", Status::Idle => "待开始" };
        let _ = tray.set_tooltip(Some(format!("极简番茄钟 · {} · {time} · {status}", snapshot.phase.label())));
    }
    if let Some(desktop) = app.try_state::<Desktop>() {
        let text = match snapshot.status { Status::Running => "暂停", Status::Paused => "继续", _ => "开始" };
        if let Ok(mut previous) = desktop.last_menu_text.lock() {
            if previous.as_str() != text { let _ = desktop.toggle.set_text(text); *previous = text.into(); }
        }
    }
}

fn dispatch<F>(app: &AppHandle, persist: bool, operation: F) -> Result<Snapshot, String>
where F: FnOnce(&mut Timer, u64) -> Result<(), String> {
    let desktop = app.state::<Desktop>();
    let (snapshot, completion, sound, result) = {
        let mut core = desktop.core.lock().map_err(|_| "计时状态暂时不可用。")?;
        let now = awake_ms();
        let date = local_date();
        let previous_date = core.timer.record.today.date.clone();
        let completion = core.timer.tick(now, &date);
        let sound = core.timer.record.settings.sound_enabled;
        let result = operation(&mut core.timer, now);
        if persist || completion.is_some() || previous_date != date || (core.timer.record.status == Status::Running && now.saturating_sub(core.last_save_ms) >= 5_000) {
            if let Err(error) = core.storage.save(&core.timer.record) {
                core.timer.warning = Some(format!("记录尚未保存：{error}。请检查磁盘空间和目录权限。"));
            }
            core.last_save_ms = now;
        }
        (core.timer.snapshot(), completion, sound, result)
    };
    publish(app, &snapshot);
    if let Some(phase) = completion { completion_notice(app, phase, sound); }
    result.map(|_| snapshot)
}

#[tauri::command]
fn get_state(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, false, |_, _| Ok(())) }
#[tauri::command]
fn start_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |timer, now| timer.start(now)) }
#[tauri::command]
fn pause_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |timer, _| timer.pause()) }
#[tauri::command]
fn resume_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |timer, now| timer.resume(now)) }
#[tauri::command]
fn reset_timer(app: AppHandle, confirmed: bool) -> Result<Snapshot, String> { dispatch(&app, true, |timer, _| timer.reset(confirmed)) }
#[tauri::command]
fn switch_phase(app: AppHandle, phase: Phase, confirmed: bool) -> Result<Snapshot, String> { dispatch(&app, true, |timer, _| timer.switch_phase(phase, confirmed)) }
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Snapshot, String> {
    settings.validate()?;
    if let Some(window) = app.get_webview_window("main") {
        window.set_always_on_top(settings.always_on_top).map_err(|error| error.to_string())?;
    }
    dispatch(&app, true, |timer, _| timer.save_settings(settings))
}
#[tauri::command]
fn set_always_on_top(app: AppHandle, enabled: bool) -> Result<Snapshot, String> {
    if let Some(window) = app.get_webview_window("main") { window.set_always_on_top(enabled).map_err(|error| error.to_string())?; }
    dispatch(&app, true, |timer, _| { timer.record.settings.always_on_top = enabled; Ok(()) })
}
#[tauri::command]
fn acknowledge_tray_hint(app: AppHandle) -> Result<Snapshot, String> {
    let snapshot = dispatch(&app, true, |timer, _| { timer.record.tray_hint_seen = true; Ok(()) })?;
    if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); }
    Ok(snapshot)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_window(app)))
        .invoke_handler(tauri::generate_handler![get_state, start_timer, pause_timer, resume_timer, reset_timer, switch_phase, save_settings, set_always_on_top, acknowledge_tray_hint])
        .setup(|app| {
            let mut window_builder = WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?;
            let directory = if cfg!(debug_assertions) {
                let path = std::env::var_os("POMODORO_DEV_DATA_DIR").map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.data/dev"));
                std::fs::create_dir_all(&path)?;
                window_builder = window_builder.data_directory(path.join("webview"));
                path
            } else { app.path().app_local_data_dir()? };
            let storage = Storage::new(directory);
            let timer = storage.load(&local_date());
            let pin = timer.record.settings.always_on_top;
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "开始", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &toggle, &quit])?;
            TrayIconBuilder::with_id("pomodoro-tray")
                .icon(app.default_window_icon().expect("应用图标缺失").clone())
                .tooltip("极简番茄钟")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event { show_window(tray.app_handle()); }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "toggle" => { let _ = dispatch(app, true, |timer, now| match timer.record.status {
                        Status::Running => timer.pause(), Status::Paused => timer.resume(now), _ => timer.start(now),
                    }); },
                    "quit" => { let _ = dispatch(app, true, |_, _| Ok(())); app.exit(0); },
                    _ => {},
                })
                .build(app)?;
            app.manage(Desktop { core: Mutex::new(Core { timer, storage, last_save_ms: awake_ms() }), toggle, last_menu_text: Mutex::new(String::new()), last_published: Mutex::new(String::new()) });
            let window = window_builder.visible(true).build()?;
            window.set_always_on_top(pin)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let _ = dispatch(&handle, false, |_, _| Ok(()));
                std::thread::sleep(std::time::Duration::from_millis(500));
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                let hint_seen = app.try_state::<Desktop>().and_then(|desktop| desktop.core.lock().ok().map(|core| core.timer.record.tray_hint_seen)).unwrap_or(false);
                if hint_seen { let _ = window.hide(); } else { let _ = app.emit("tray-hint", ()); }
            }
        })
        .build(tauri::generate_context!())
        .expect("无法启动极简番茄钟");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit) {
            let _ = dispatch(app, true, |_, _| Ok(()));
        }
    });
}

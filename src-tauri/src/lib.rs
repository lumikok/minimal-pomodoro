mod platform;
mod storage;
mod timer;
mod viewport;

use platform::{awake_ms, local_date};
use std::{sync::{Mutex, atomic::{AtomicBool, AtomicU8, Ordering}}, time::{Duration, Instant}};
use storage::Storage;
use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, AppHandle, Emitter, Manager, WebviewWindowBuilder};
use timer::{Counters, Phase, Position, Settings, Snapshot, Status, Timer};

struct Core {
    timer: Timer, storage: Storage, last_save_ms: u64,
    ring_until: Option<Instant>, position_dirty: bool, position_changed_at: u64,
}
struct Desktop {
    core: Mutex<Core>, toggle: MenuItem<tauri::Wry>,
    last_menu_text: Mutex<String>, last_published: Mutex<String>,
    expanded: AtomicBool, layout: AtomicU8, topmost: AtomicBool, ignored_position: Mutex<Option<Position>>,
}

fn apply_layout(app: &AppHandle, snapshot: &Snapshot, repair: bool) {
    let Some(window) = app.get_webview_window("main") else { return; };
    let desktop = app.state::<Desktop>();
    let mode = if snapshot.reminder_pending { 2 } else if snapshot.expanded { 1 } else { 0 };
    let changed = desktop.layout.swap(mode, Ordering::SeqCst) != mode;
    let topmost = snapshot.reminder_pending || snapshot.settings.always_on_top;
    if desktop.topmost.swap(topmost, Ordering::SeqCst) != topmost { let _ = window.set_always_on_top(topmost); }
    if !changed && !repair { return; }
    let (width, height) = match mode { 2 => (320., 224.), 1 => (360., 520.), _ => (220., 100.) };
    let areas: Vec<viewport::Area> = window.available_monitors().unwrap_or_default().iter().map(|m| {
        let area = m.work_area();
        viewport::Area { x: area.position.x, y: area.position.y, width: area.size.width, height: area.size.height, scale: m.scale_factor() }
    }).collect();
    let position = viewport::place(snapshot.window.position, &areas, width, height);
    if let Ok(mut ignored) = desktop.ignored_position.lock() { *ignored = Some(position); }
    if changed { let _ = window.set_size(tauri::LogicalSize::new(width, height)); }
    let current = window.outer_position().ok();
    if current.is_none_or(|p| p.x != position.x || p.y != position.y) {
        let _ = window.set_position(tauri::PhysicalPosition::new(position.x, position.y));
    }
    let _ = window.set_always_on_top(snapshot.reminder_pending || snapshot.settings.always_on_top);
}

fn show_window(app: &AppHandle) {
    if let Ok(snapshot) = dispatch(app, false, |_, _| Ok(())) { apply_layout(app, &snapshot, true); }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show(); let _ = window.unminimize(); let _ = window.set_focus();
    }
}

fn completion_notice(app: &AppHandle, phase: Phase) {
    if let Some(window) = app.get_webview_window("main") { platform::show_without_focus(&window); }
    let body = if phase == Phase::Focus { "专注已完成，准备好后手动开始休息。" } else { "休息结束，准备好后手动开始专注。" };
    let app_id = if cfg!(debug_assertions) { tauri_winrt_notification::Toast::POWERSHELL_APP_ID } else { &app.config().identifier };
    if let Err(error) = tauri_winrt_notification::Toast::new(app_id).title(&format!("{}完成", phase.label())).text1(body).sound(None).show() {
        if let Ok(mut core) = app.state::<Desktop>().core.lock() { core.timer.warning = Some(format!("系统通知未能发送，浮窗仍会提醒：{error}")); }
    }
}

fn publish(app: &AppHandle, snapshot: &Snapshot) {
    let desktop = app.state::<Desktop>();
    let mut comparable = snapshot.clone(); comparable.revision = 0;
    if let Ok(key) = serde_json::to_string(&comparable) {
        if let Ok(mut previous) = desktop.last_published.lock() {
            if *previous == key { return; } *previous = key;
        }
    }
    apply_layout(app, snapshot, false);
    let _ = app.emit("timer-state", snapshot);
    if let Some(tray) = app.tray_by_id("pomodoro-tray") {
        let seconds = snapshot.remaining_seconds;
        let time = if seconds >= 3600 { format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60) }
            else { format!("{:02}:{:02}", seconds / 60, seconds % 60) };
        let status = match snapshot.status { Status::Running => "计时中", Status::Paused => "已暂停", Status::Completed => "已完成", Status::Idle => "待开始" };
        let _ = tray.set_tooltip(Some(format!("极简番茄钟 · {} · {time} · {status}", snapshot.phase.label())));
    }
    let text = match snapshot.status { Status::Running => "暂停", Status::Paused => "继续", _ => "开始下一段" };
    if let Ok(mut previous) = desktop.last_menu_text.lock() {
        if previous.as_str() != text { let _ = desktop.toggle.set_text(text); *previous = text.into(); }
    }
}

fn dispatch<F>(app: &AppHandle, persist: bool, operation: F) -> Result<Snapshot, String>
where F: FnOnce(&mut Timer, u64) -> Result<(), String> {
    let desktop = app.state::<Desktop>();
    let (mut snapshot, completion, result) = {
        let mut core = desktop.core.lock().map_err(|_| "计时状态暂时不可用。")?;
        let now = awake_ms(); let date = local_date();
        let previous_date = core.timer.record.today.date.clone();
        let completion = core.timer.tick(now, &date);
        let result = operation(&mut core.timer, now);
        if completion.is_some() && core.timer.record.reminder_pending && core.timer.record.settings.sound_enabled {
            platform::start_chime(); core.ring_until = Some(Instant::now() + Duration::from_secs(10));
        }
        if core.ring_until.is_some() && (!core.timer.record.reminder_pending || !core.timer.record.settings.sound_enabled
            || core.ring_until.is_some_and(|until| Instant::now() >= until)) {
            platform::stop_chime(); core.ring_until = None;
        }
        if persist || completion.is_some() || previous_date != date
            || (core.position_dirty && now.saturating_sub(core.position_changed_at) >= 1000)
            || (core.timer.record.status == Status::Running && now.saturating_sub(core.last_save_ms) >= 5000) {
            match core.storage.save(&core.timer.record) {
                Ok(()) => core.position_dirty = false,
                Err(error) => core.timer.warning = Some(format!("记录尚未保存：{error}")),
            }
            core.last_save_ms = now;
        }
        let mut snapshot = core.timer.snapshot(); snapshot.ringing = core.ring_until.is_some();
        (snapshot, completion, result)
    };
    snapshot.expanded = desktop.expanded.load(Ordering::SeqCst);
    publish(app, &snapshot);
    if let Some(phase) = completion.filter(|_| snapshot.reminder_pending) { completion_notice(app, phase); }
    result.map(|_| snapshot)
}

#[tauri::command]
fn get_state(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, false, |_, _| Ok(())) }
#[tauri::command]
fn start_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |t, now| t.start(now)) }
#[tauri::command]
fn pause_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |t, _| t.pause()) }
#[tauri::command]
fn resume_timer(app: AppHandle) -> Result<Snapshot, String> { dispatch(&app, true, |t, now| t.resume(now)) }
#[tauri::command]
fn reset_timer(app: AppHandle, confirmed: bool) -> Result<Snapshot, String> { dispatch(&app, true, |t, _| t.reset(confirmed)) }
#[tauri::command]
fn switch_phase(app: AppHandle, phase: Phase, confirmed: bool) -> Result<Snapshot, String> { dispatch(&app, true, |t, _| t.switch_phase(phase, confirmed)) }
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Snapshot, String> { dispatch(&app, true, |t, _| t.save_settings(settings)) }
#[tauri::command]
fn set_always_on_top(app: AppHandle, enabled: bool) -> Result<Snapshot, String> {
    dispatch(&app, true, |t, _| { t.record.settings.always_on_top = enabled; Ok(()) })
}
#[tauri::command]
fn set_expanded(app: AppHandle, expanded: bool) -> Result<Snapshot, String> {
    app.state::<Desktop>().expanded.store(expanded, Ordering::SeqCst);
    dispatch(&app, false, |_, _| Ok(()))
}
#[tauri::command]
fn acknowledge_completion(app: AppHandle) -> Result<Snapshot, String> {
    app.state::<Desktop>().expanded.store(false, Ordering::SeqCst);
    dispatch(&app, true, |t, _| { t.record.reminder_pending = false; Ok(()) })
}
#[tauri::command]
fn silence_chime(app: AppHandle) -> Result<Snapshot, String> {
    if let Ok(mut core) = app.state::<Desktop>().core.lock() { platform::stop_chime(); core.ring_until = None; }
    dispatch(&app, false, |_, _| Ok(()))
}
#[tauri::command]
fn save_history(app: AppHandle, history: Counters) -> Result<Snapshot, String> { dispatch(&app, true, |t, _| t.set_history(history)) }
#[tauri::command]
fn save_opacity(app: AppHandle, opacity: u8) -> Result<Snapshot, String> {
    if !(40..=100).contains(&opacity) { return Err("底色不透明度需要为 40–100%。".into()); }
    dispatch(&app, true, |t, _| { t.record.window.opacity = opacity; Ok(()) })
}
#[tauri::command]
fn hide_window(app: AppHandle) -> Result<Snapshot, String> {
    let snapshot = acknowledge_completion(app.clone())?;
    if let Some(window) = app.get_webview_window("main") { window.hide().map_err(|e| e.to_string())?; }
    Ok(snapshot)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_window(app)))
        .invoke_handler(tauri::generate_handler![get_state, start_timer, pause_timer, resume_timer, reset_timer,
            switch_phase, save_settings, set_always_on_top, set_expanded, acknowledge_completion, silence_chime, save_history, save_opacity, hide_window])
        .setup(|app| {
            let mut builder = WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?;
            let directory = if cfg!(debug_assertions) || cfg!(feature = "acceptance") {
                let path = std::env::var_os("POMODORO_DEV_DATA_DIR").map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.data/dev"));
                std::fs::create_dir_all(&path)?; builder = builder.data_directory(path.join("webview")); path
            } else { app.path().app_local_data_dir()? };
            let storage = Storage::new(directory); let timer = storage.load(&local_date());
            let pending = timer.record.reminder_pending;
            // Persist the migrated record immediately, including when no timer is running.
            let save_error = storage.save(&timer.record).err();
            let show = MenuItem::with_id(app, "show", "显示浮窗", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "开始", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            TrayIconBuilder::with_id("pomodoro-tray").icon(app.default_window_icon().expect("应用图标缺失").clone())
                .tooltip("极简番茄钟").menu(&Menu::with_items(app, &[&show, &toggle, &quit])?).show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event { show_window(tray.app_handle()); }
                }).on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "toggle" => { let _ = dispatch(app, true, |t, now| match t.record.status {
                        Status::Running => t.pause(), Status::Paused => t.resume(now), _ => t.start(now),
                    }); },
                    "quit" => { platform::stop_chime(); let _ = dispatch(app, true, |_, _| Ok(())); app.exit(0); }, _ => {},
                }).build(app)?;
            let mut timer = timer;
            if let Some(error) = save_error { timer.warning = Some(format!("升级记录尚未保存：{error}")); }
            app.manage(Desktop { core: Mutex::new(Core { timer, storage, last_save_ms: awake_ms(), ring_until: None,
                    position_dirty: false, position_changed_at: 0 }), toggle, last_menu_text: Mutex::new(String::new()),
                last_published: Mutex::new(String::new()), expanded: AtomicBool::new(false), layout: AtomicU8::new(255), topmost: AtomicBool::new(false), ignored_position: Mutex::new(None) });
            let window = builder.visible(false).build()?;
            let snapshot = dispatch(app.handle(), false, |_, _| Ok(()))?;
            apply_layout(app.handle(), &snapshot, true);
            if pending { platform::show_without_focus(&window); } else { window.show()?; }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut steps = 0;
                loop {
                    if let Ok(snapshot) = dispatch(&handle, false, |_, _| Ok(())) {
                        steps += 1;
                        let moving = handle.state::<Desktop>().core.lock().map(|c| c.position_dirty).unwrap_or(true);
                        if steps % 10 == 0 && !moving { apply_layout(&handle, &snapshot, true); }
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            });
            Ok(())
        }).on_window_event(|window, event| {
            if let tauri::WindowEvent::Moved(position) = event {
                // Ignore queued intermediate moves from resize/monitor changes.
                if window.outer_position().is_ok_and(|current| current != *position) { return; }
                if let Some(desktop) = window.app_handle().try_state::<Desktop>() {
                    let point = Position { x: position.x, y: position.y };
                    if let Ok(mut ignored) = desktop.ignored_position.lock() {
                        if *ignored == Some(point) { *ignored = None; return; }
                    }
                    if let Ok(mut core) = desktop.core.lock() {
                        if core.timer.record.window.position != Some(point) {
                            core.timer.record.window.position = Some(point); core.position_dirty = true; core.position_changed_at = awake_ms();
                        }
                    }
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close(); let _ = hide_window(window.app_handle().clone());
            }
        }).build(tauri::generate_context!()).expect("无法启动极简番茄钟");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit) {
            platform::stop_chime(); let _ = dispatch(app, true, |_, _| Ok(()));
        }
    });
}

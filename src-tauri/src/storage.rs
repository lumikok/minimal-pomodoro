use crate::platform::atomic_replace;
use crate::timer::{Record, Timer};
use std::{fs, io::Write, path::PathBuf, sync::atomic::{AtomicBool, Ordering}};

pub struct Storage {
    directory: PathBuf,
    writable: AtomicBool,
}

impl Storage {
    pub fn new(directory: PathBuf) -> Self { Self { directory, writable: AtomicBool::new(true) } }

    pub fn load(&self, date: &str) -> Timer {
        let path = self.directory.join("state.json");
        if !path.exists() { return Timer::new(date); }
        let loaded = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).map_err(|error| error.to_string()))
            .and_then(|record: Record| {
                record.validate()?;
                if record.version == 1 {
                    let backup = self.directory.join(format!("state.pre-v0.2-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S-%f")));
                    if let Err(error) = fs::copy(&path, &backup) {
                        self.writable.store(false, Ordering::SeqCst);
                        return Timer::restore(record, date).map(|mut timer| {
                            timer.warning = Some(format!("升级备份失败，原文件未覆盖；当前改动不能保存：{error}")); timer
                        });
                    }
                }
                Timer::restore(record, date)
            });
        match loaded {
            Ok(timer) => timer,
            Err(error) => {
                let backup = self.directory.join(format!("state.invalid-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S-%f")));
                let preservation = fs::rename(&path, &backup);
                let mut timer = Timer::new(date);
                timer.warning = Some(match preservation {
                    Ok(()) => format!("上次记录无法读取，已保留为 {}。本次使用默认设置。原因：{error}", backup.display()),
                    Err(backup_error) => format!("上次记录无法读取且无法备份，本次不会覆盖原记录。原因：{backup_error}"),
                });
                timer
            }
        }
    }

    pub fn save(&self, record: &Record) -> Result<(), String> {
        if !self.writable.load(Ordering::SeqCst) { return Err("升级备份未成功，已停止覆盖原记录。".into()); }
        fs::create_dir_all(&self.directory).map_err(|error| format!("无法创建数据目录：{error}"))?;
        let path = self.directory.join("state.json");
        // Preserve an unreadable original if recovery could not rename it.
        if path.exists() {
            let bytes = fs::read(&path).map_err(|error| format!("原记录无法读取，已停止覆盖：{error}"))?;
            let previous = serde_json::from_slice::<Record>(&bytes).map_err(|error| format!("原记录尚未成功备份，已停止覆盖：{error}"))?;
            previous.validate().map_err(|error| format!("原记录尚未成功备份，已停止覆盖：{error}"))?;
        }
        record.validate()?;
        let bytes = serde_json::to_vec_pretty(record).map_err(|error| error.to_string())?;
        let temp = self.directory.join("state.json.tmp");
        let mut file = fs::File::create(&temp).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        atomic_replace(&temp, &path).map_err(|error| format!("无法保存记录：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::Status;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::var_os("POMODORO_DEV_DATA_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
            Self(root.join(format!("storage-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst))))
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn saves_replaces_and_restores_running_as_paused() {
        let directory = TestDirectory::new();
        let storage = Storage::new(directory.0.clone());
        let mut timer = Timer::new("2026-10-07");
        storage.save(&timer.record).unwrap();
        timer.start(0).unwrap(); timer.tick(10_000, "2026-10-07");
        storage.save(&timer.record).unwrap();
        let restored = storage.load("2026-10-07");
        assert_eq!(restored.record.status, Status::Paused);
        assert_eq!(restored.record.remaining_ms, 1_490_000);
        assert!(!directory.0.join("state.json.tmp").exists());
    }

    #[test]
    fn corrupt_record_is_preserved_and_new_record_can_be_saved() {
        let directory = TestDirectory::new();
        fs::create_dir_all(&directory.0).unwrap();
        fs::write(directory.0.join("state.json"), b"{broken").unwrap();
        let storage = Storage::new(directory.0.clone());
        let timer = storage.load("2026-10-07");
        assert!(timer.warning.is_some());
        let backup = fs::read_dir(&directory.0).unwrap().next().unwrap().unwrap();
        assert_eq!(fs::read(backup.path()).unwrap(), b"{broken");
        storage.save(&timer.record).unwrap();
    }
    #[test]
    fn migration_backup_is_exact_and_totals_persist_once() {
        let directory = TestDirectory::new(); fs::create_dir_all(&directory.0).unwrap();
        let mut record = Timer::new("2026-10-07").record; record.version = 1;
        record.today.completed_count = 2; record.today.focus_minutes = 50;
        let mut json = serde_json::to_value(record).unwrap();
        for key in ["lifetime", "window", "reminderPending"] { json.as_object_mut().unwrap().remove(key); }
        let bytes = serde_json::to_vec(&json).unwrap(); fs::write(directory.0.join("state.json"), &bytes).unwrap();
        let storage = Storage::new(directory.0.clone()); let timer = storage.load("2026-10-08");
        let backup = fs::read_dir(&directory.0).unwrap().find_map(|e| { let e = e.unwrap(); e.file_name().to_string_lossy().starts_with("state.pre-v0.2").then_some(e.path()) }).unwrap();
        assert_eq!(fs::read(backup).unwrap(), bytes);
        storage.save(&timer.record).unwrap(); let again = storage.load("2026-10-09");
        assert_eq!(again.record.lifetime.completed_count, 2); assert_eq!(again.record.today.completed_count, 0);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
}

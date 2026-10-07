#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryUnbiasedInterruptTime(time: *mut u64) -> i32;
    fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBeep(kind: u32) -> i32;
}

pub fn awake_ms() -> u64 {
    #[cfg(windows)]
    {
        let mut ticks = 0u64;
        // Windows documents this value as 100 ns units, excluding suspend time.
        let succeeded = unsafe { QueryUnbiasedInterruptTime(&mut ticks) };
        assert_ne!(succeeded, 0, "无法读取 Windows 单调时钟");
        ticks / 10_000
    }
    #[cfg(not(windows))]
    {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_millis() as u64
    }
}

pub fn local_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn short_beep() {
    #[cfg(windows)]
    unsafe {
        // Use the user's short system sound; no media downloads or background player.
        MessageBeep(0x40);
    }
}

pub fn atomic_replace(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
        // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH; same-directory temp file.
        if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0x1 | 0x8) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    { std::fs::rename(source, destination) }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "专注",
            Self::ShortBreak => "短休息",
            Self::LongBreak => "长休息",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Idle,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub sound_enabled: bool,
    pub always_on_top: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            sound_enabled: true,
            always_on_top: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if [self.focus_minutes, self.short_break_minutes, self.long_break_minutes]
            .iter()
            .any(|value| !(1..=180).contains(value))
        {
            return Err("时长需要是 1–180 的整数分钟。".into());
        }
        Ok(())
    }

    fn duration_ms(&self, phase: Phase) -> u64 {
        let minutes = match phase {
            Phase::Focus => self.focus_minutes,
            Phase::ShortBreak => self.short_break_minutes,
            Phase::LongBreak => self.long_break_minutes,
        };
        u64::from(minutes) * 60_000
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Today {
    pub date: String,
    pub completed_count: u32,
    pub focus_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub version: u32,
    pub settings: Settings,
    pub phase: Phase,
    pub status: Status,
    pub total_ms: u64,
    pub remaining_ms: u64,
    pub today: Today,
    pub cycle_count: u8,
    pub tray_hint_seen: bool,
    #[serde(default)]
    pub lifetime: Lifetime,
    #[serde(default)]
    pub window: WindowPreferences,
    #[serde(default)]
    pub reminder_pending: bool,
}

impl Record {
    pub fn validate(&self) -> Result<(), String> {
        self.settings.validate()?;
        if self.version != 1 && self.version != 2 {
            return Err("不支持的数据版本。".into());
        }
        if !(60_000..=10_800_000).contains(&self.total_ms)
            || self.total_ms % 60_000 != 0
            || self.remaining_ms > self.total_ms
            || self.cycle_count > 3
            || chrono::NaiveDate::parse_from_str(&self.today.date, "%Y-%m-%d").is_err()
            || (self.status == Status::Completed && self.remaining_ms != 0)
            || (self.status != Status::Completed && self.remaining_ms == 0)
            || (self.status == Status::Idle && self.remaining_ms != self.total_ms)
            || self.today.focus_minutes < self.today.completed_count
            || u64::from(self.today.focus_minutes) > u64::from(self.today.completed_count) * 180
        {
            return Err("计时记录包含无效字段。".into());
        }
        if self.version == 2 {
            Counters { completed_count: self.lifetime.completed_count, focus_minutes: self.lifetime.focus_minutes }.validate()?;
            self.lifetime.history.validate()?;
            if chrono::NaiveDate::parse_from_str(&self.lifetime.since, "%Y-%m-%d").is_err()
                || self.lifetime.completed_count < u64::from(self.today.completed_count)
                || self.lifetime.focus_minutes < u64::from(self.today.focus_minutes)
                || !(40..=100).contains(&self.window.opacity)
                || self.window.position.is_some_and(|p| p.x.unsigned_abs() > 1_000_000 || p.y.unsigned_abs() > 1_000_000)
                || (self.reminder_pending && self.status != Status::Completed) { return Err("累计或浮窗记录无效。".into()); }
        }
        Ok(())
    }
    pub fn migrate(&mut self) {
        if self.version == 1 {
            self.lifetime = Lifetime { since: self.today.date.clone(), completed_count: u64::from(self.today.completed_count),
                focus_minutes: u64::from(self.today.focus_minutes), history: Counters::default() };
            self.window = WindowPreferences::default();
            self.settings.always_on_top = true;
            self.reminder_pending = false;
            self.version = 2;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters { pub completed_count: u64, pub focus_minutes: u64 }
impl Counters {
    pub fn validate(&self) -> Result<(), String> {
        if self.completed_count > 1_000_000_000 || self.focus_minutes < self.completed_count
            || self.focus_minutes > self.completed_count * 180 { return Err("次数与专注分钟不一致（每次 1–180 分钟）。".into()); }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lifetime {
    pub since: String,
    pub completed_count: u64,
    pub focus_minutes: u64,
    pub history: Counters,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position { pub x: i32, pub y: i32 }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPreferences { pub opacity: u8, pub position: Option<Position> }
impl Default for WindowPreferences { fn default() -> Self { Self { opacity: 80, position: None } } }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub status: Status,
    pub remaining_seconds: u64,
    pub total_seconds: u64,
    pub next_phase: Phase,
    pub settings: Settings,
    pub today: Today,
    pub cycle_count: u8,
    pub tray_hint_seen: bool,
    pub warning: Option<String>,
    pub revision: u64,
    pub lifetime: Lifetime,
    pub window: WindowPreferences,
    pub reminder_pending: bool,
    pub expanded: bool,
    pub ringing: bool,
}

pub struct Timer {
    pub record: Record,
    pub warning: Option<String>,
    anchor_ms: Option<u64>,
    revision: u64,
}

impl Timer {
    pub fn new(date: &str) -> Self {
        let settings = Settings::default();
        let total_ms = settings.duration_ms(Phase::Focus);
        Self {
            record: Record {
                version: 2,
                settings,
                phase: Phase::Focus,
                status: Status::Idle,
                total_ms,
                remaining_ms: total_ms,
                today: Today { date: date.into(), completed_count: 0, focus_minutes: 0 },
                cycle_count: 0,
                tray_hint_seen: false,
                lifetime: Lifetime { since: date.into(), ..Lifetime::default() },
                window: WindowPreferences::default(),
                reminder_pending: false,
            },
            warning: None,
            anchor_ms: None,
            revision: 0,
        }
    }

    pub fn restore(mut record: Record, date: &str) -> Result<Self, String> {
        record.validate()?;
        record.migrate();
        if record.status == Status::Running {
            record.status = Status::Paused;
        }
        let mut timer = Self { record, warning: None, anchor_ms: None, revision: 0 };
        timer.update_date(date);
        Ok(timer)
    }

    pub fn snapshot(&mut self) -> Snapshot {
        self.revision = self.revision.wrapping_add(1);
        Snapshot {
            phase: self.record.phase,
            status: self.record.status,
            remaining_seconds: self.record.remaining_ms.div_ceil(1_000),
            total_seconds: self.record.total_ms / 1_000,
            next_phase: self.next_phase(),
            settings: self.record.settings.clone(),
            today: self.record.today.clone(),
            cycle_count: self.record.cycle_count,
            tray_hint_seen: self.record.tray_hint_seen,
            warning: self.warning.clone(),
            revision: self.revision,
            lifetime: self.record.lifetime.clone(),
            window: self.record.window.clone(),
            reminder_pending: self.record.reminder_pending,
            expanded: false,
            ringing: false,
        }
    }

    pub fn next_phase(&self) -> Phase {
        match self.record.phase {
            Phase::Focus if self.record.cycle_count == 0 => Phase::LongBreak,
            Phase::Focus => Phase::ShortBreak,
            _ => Phase::Focus,
        }
    }

    fn update_date(&mut self, date: &str) {
        if self.record.today.date != date {
            self.record.today = Today { date: date.into(), completed_count: 0, focus_minutes: 0 };
        }
    }

    // `now_ms` is an awake-only monotonic time supplied by the Windows adapter.
    // Wall-clock time is used exclusively for attribution to a local date.
    pub fn tick(&mut self, now_ms: u64, date: &str) -> Option<Phase> {
        self.update_date(date);
        if self.record.status != Status::Running {
            return None;
        }
        let previous = self.anchor_ms.replace(now_ms).unwrap_or(now_ms);
        self.record.remaining_ms = self.record.remaining_ms.saturating_sub(now_ms.saturating_sub(previous));
        if self.record.remaining_ms > 0 {
            return None;
        }
        self.record.status = Status::Completed;
        self.record.reminder_pending = true;
        self.anchor_ms = None;
        if self.record.phase == Phase::Focus {
            self.record.today.completed_count = self.record.today.completed_count.saturating_add(1);
            self.record.today.focus_minutes = self.record.today.focus_minutes.saturating_add((self.record.total_ms / 60_000) as u32);
            self.record.cycle_count = (self.record.cycle_count + 1) % 4;
            self.record.lifetime.completed_count += 1;
            self.record.lifetime.focus_minutes += self.record.total_ms / 60_000;
        }
        Some(self.record.phase)
    }

    pub fn start(&mut self, now_ms: u64) -> Result<(), String> {
        match self.record.status {
            Status::Running | Status::Paused => return Err("当前计时已经开始。".into()),
            Status::Completed => self.prepare(self.next_phase()),
            Status::Idle => {},
        }
        self.record.status = Status::Running;
        self.anchor_ms = Some(now_ms);
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.record.status == Status::Completed { return Ok(()); }
        if self.record.status != Status::Running { return Err("当前没有运行中的计时。".into()); }
        self.record.status = Status::Paused;
        self.anchor_ms = None;
        Ok(())
    }

    pub fn resume(&mut self, now_ms: u64) -> Result<(), String> {
        if self.record.status != Status::Paused { return Err("当前没有暂停的计时。".into()); }
        self.record.status = Status::Running;
        self.anchor_ms = Some(now_ms);
        Ok(())
    }

    pub fn reset(&mut self, confirmed: bool) -> Result<(), String> {
        self.require_confirmation(confirmed)?;
        self.prepare(self.record.phase);
        Ok(())
    }

    pub fn switch_phase(&mut self, phase: Phase, confirmed: bool) -> Result<(), String> {
        if phase == self.record.phase && self.record.status != Status::Completed { return Ok(()); }
        self.require_confirmation(confirmed)?;
        self.prepare(phase);
        Ok(())
    }

    fn require_confirmation(&self, confirmed: bool) -> Result<(), String> {
        if matches!(self.record.status, Status::Running | Status::Paused) && !confirmed {
            return Err("请先确认放弃当前未完成的计时。".into());
        }
        Ok(())
    }

    fn prepare(&mut self, phase: Phase) {
        self.record.phase = phase;
        self.record.status = Status::Idle;
        self.record.total_ms = self.record.settings.duration_ms(phase);
        self.record.remaining_ms = self.record.total_ms;
        self.record.reminder_pending = false;
        self.anchor_ms = None;
    }

    pub fn set_history(&mut self, history: Counters) -> Result<(), String> {
        history.validate()?;
        self.record.lifetime.history = history;
        Ok(())
    }

    pub fn save_settings(&mut self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        self.record.settings = settings;
        if self.record.status == Status::Idle { self.prepare(self.record.phase); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DAY: &str = "2026-10-07";

    fn short_timer() -> Timer {
        let mut timer = Timer::new(DAY);
        timer.save_settings(Settings { focus_minutes: 1, ..Settings::default() }).unwrap();
        timer
    }

    #[test]
    fn pause_resume_excludes_paused_time() {
        let mut timer = short_timer();
        timer.start(100).unwrap();
        timer.tick(10_100, DAY);
        timer.pause().unwrap();
        timer.tick(500_000, DAY);
        assert_eq!(timer.record.remaining_ms, 50_000);
        timer.resume(500_000).unwrap();
        assert_eq!(timer.tick(549_999, DAY), None);
        assert_eq!(timer.tick(550_000, DAY), Some(Phase::Focus));
        assert_eq!(timer.record.today.focus_minutes, 1);
    }

    #[test]
    fn completion_is_counted_once_and_does_not_auto_start() {
        let mut timer = short_timer();
        timer.start(0).unwrap();
        assert_eq!(timer.tick(60_000, DAY), Some(Phase::Focus));
        assert_eq!(timer.tick(120_000, DAY), None);
        assert_eq!(timer.record.today.completed_count, 1);
        assert_eq!(timer.record.status, Status::Completed);
        assert_eq!(timer.next_phase(), Phase::ShortBreak);
    }

    #[test]
    fn fourth_focus_recommends_long_break_across_days() {
        let mut timer = short_timer();
        for round in 1..=4 {
            timer.switch_phase(Phase::Focus, true).unwrap();
            timer.start(0).unwrap();
            timer.tick(60_000, if round < 4 { DAY } else { "2026-10-08" });
            assert_eq!(timer.next_phase(), if round == 4 { Phase::LongBreak } else { Phase::ShortBreak });
        }
        assert_eq!(timer.record.today.completed_count, 1);
        assert_eq!(timer.record.cycle_count, 0);
    }

    #[test]
    fn break_does_not_count_and_recommends_focus() {
        let mut timer = short_timer();
        timer.switch_phase(Phase::ShortBreak, false).unwrap();
        timer.start(0).unwrap();
        timer.tick(300_000, DAY);
        assert_eq!(timer.record.today.completed_count, 0);
        assert_eq!(timer.next_phase(), Phase::Focus);
    }

    #[test]
    fn reset_and_switch_need_confirmation_and_do_not_count() {
        let mut timer = short_timer();
        timer.start(0).unwrap();
        timer.tick(10_000, DAY);
        assert!(timer.reset(false).is_err());
        assert!(timer.switch_phase(Phase::LongBreak, false).is_err());
        timer.reset(true).unwrap();
        assert_eq!(timer.record.today.completed_count, 0);
        assert_eq!(timer.record.remaining_ms, 60_000);
    }

    #[test]
    fn settings_only_change_next_started_segment() {
        let mut timer = short_timer();
        timer.start(0).unwrap();
        timer.save_settings(Settings { focus_minutes: 50, ..Settings::default() }).unwrap();
        timer.tick(60_000, DAY);
        assert_eq!(timer.record.today.focus_minutes, 1);
        timer.switch_phase(Phase::Focus, false).unwrap();
        assert_eq!(timer.record.total_ms, 3_000_000);
    }

    #[test]
    fn completed_segment_starts_recommended_phase_only_on_request() {
        let mut timer = short_timer();
        timer.start(0).unwrap(); timer.tick(60_000, DAY);
        timer.start(90_000).unwrap();
        assert_eq!(timer.record.phase, Phase::ShortBreak);
        assert_eq!(timer.record.remaining_ms, 300_000);
    }

    #[test]
    fn midnight_attribution_and_idle_date_rollover() {
        let mut timer = short_timer();
        timer.start(0).unwrap();
        timer.tick(50_000, DAY);
        timer.tick(60_000, "2026-10-08");
        assert_eq!(timer.record.today.date, "2026-10-08");
        assert_eq!(timer.record.today.completed_count, 1);
        timer.tick(70_000, "2026-10-09");
        assert_eq!(timer.record.today.completed_count, 0);
    }

    #[test]
    fn awake_clock_freezes_during_sleep_and_wall_date_does_not_drive_timer() {
        let mut timer = short_timer();
        timer.start(1_000).unwrap();
        timer.tick(11_000, DAY);
        timer.tick(11_000, "2030-01-01");
        assert_eq!(timer.record.remaining_ms, 50_000);
        timer.tick(12_000, DAY);
        assert_eq!(timer.record.remaining_ms, 49_000);
    }

    #[test]
    fn restarting_running_record_pauses_without_offline_credit() {
        let mut timer = short_timer();
        timer.start(0).unwrap(); timer.tick(10_000, DAY);
        let mut restored = Timer::restore(timer.record, DAY).unwrap();
        restored.tick(1_000_000, DAY);
        assert_eq!(restored.record.status, Status::Paused);
        assert_eq!(restored.record.remaining_ms, 50_000);
        assert_eq!(restored.record.today.completed_count, 0);
    }

    #[test]
    fn invalid_settings_and_record_are_rejected() {
        for minutes in [0, 181, u32::MAX] {
            assert!(Settings { focus_minutes: minutes, ..Settings::default() }.validate().is_err());
        }
        for minutes in [1, 180] {
            assert!(Settings { focus_minutes: minutes, ..Settings::default() }.validate().is_ok());
        }
        let mut record = short_timer().record;
        record.remaining_ms = record.total_ms + 1;
        assert!(record.validate().is_err());
        record.remaining_ms = 0;
        assert!(record.validate().is_err());
    }

    #[test]
    fn display_rounds_up_and_snapshots_are_ordered() {
        let mut timer = short_timer();
        timer.start(0).unwrap(); timer.tick(1, DAY);
        let first = timer.snapshot(); let second = timer.snapshot();
        assert_eq!(first.remaining_seconds, 60);
        assert!(second.revision > first.revision);
    }

    #[test]
    fn lifetime_survives_midnight_and_restart_without_duplicate_completion() {
        let mut timer = short_timer(); timer.start(0).unwrap(); timer.tick(60_000, DAY);
        timer.tick(80_000, "2026-10-08");
        assert_eq!(timer.record.today.completed_count, 0);
        assert_eq!(timer.record.lifetime.completed_count, 1);
        assert_eq!(timer.record.lifetime.focus_minutes, 1);
        let mut restored = Timer::restore(timer.record, "2026-10-08").unwrap();
        assert!(restored.record.reminder_pending);
        restored.tick(100_000, "2026-10-08");
        assert_eq!(restored.record.lifetime.completed_count, 1);
    }
    #[test]
    fn history_replaces_offset_without_changing_today_or_clock() {
        let mut timer = short_timer(); timer.start(0).unwrap(); timer.tick(10_000, DAY);
        for _ in 0..2 { timer.set_history(Counters { completed_count: 3, focus_minutes: 75 }).unwrap(); }
        assert_eq!(timer.record.lifetime.history.completed_count, 3);
        assert_eq!(timer.record.lifetime.completed_count, 0);
        assert_eq!(timer.record.today.completed_count, 0);
        assert_eq!(timer.record.remaining_ms, 50_000);
        assert!(timer.set_history(Counters { completed_count: 0, focus_minutes: 1 }).is_err());
        assert_eq!(timer.record.lifetime.history.focus_minutes, 75);
    }
    #[test]
    fn legacy_migration_keeps_retained_day_before_rollover() {
        let mut record = short_timer().record; record.version = 1;
        record.today.completed_count = 2; record.today.focus_minutes = 50;
        record.settings.sound_enabled = false;
        let timer = Timer::restore(record, "2026-10-08").unwrap();
        assert_eq!(timer.record.version, 2);
        assert_eq!(timer.record.lifetime.completed_count, 2);
        assert_eq!(timer.record.lifetime.focus_minutes, 50);
        assert_eq!(timer.record.lifetime.since, DAY);
        assert_eq!(timer.record.today.completed_count, 0);
        assert!(!timer.record.settings.sound_enabled);
        assert!(timer.record.settings.always_on_top);
    }
    #[test]
    fn dismiss_and_next_segment_do_not_count_or_auto_start() {
        let mut timer = short_timer(); timer.start(0).unwrap(); timer.tick(60_000, DAY);
        timer.record.reminder_pending = false; timer.tick(70_000, DAY);
        assert_eq!(timer.record.status, Status::Completed);
        assert_eq!(timer.record.lifetime.completed_count, 1);
        timer.start(80_000).unwrap();
        assert!(!timer.record.reminder_pending);
        assert_eq!(timer.record.phase, Phase::ShortBreak);
    }
    #[test]
    fn malformed_v2_totals_and_window_preferences_are_rejected() {
        let mut record = short_timer().record; record.window.opacity = 0;
        assert!(record.validate().is_err()); record.window.opacity = 80;
        record.lifetime.focus_minutes = 25; assert!(record.validate().is_err());
    }
}

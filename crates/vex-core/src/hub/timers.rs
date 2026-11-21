//! Shared timer engine for the Pomodoro and Countdown widgets.
//!
//! The rule that keeps timers honest: **a running timer stores the absolute wall-clock time it ends
//! (unix milliseconds), never a tick count.** Remaining time is always `ends_at - now`, so it is right after
//! the window was closed, after the PC slept and after VEX restarted. Every operation takes `now_ms` so
//! tests drive a fake clock. Nothing here sleeps or spawns threads; the shell's scheduler asks
//! `next_deadline()` and calls `poll()`.
//!
//! Known limit: this uses the system clock, so if the user changes the clock while a timer runs, the timer
//! follows the new clock.

use serde::{Deserialize, Serialize};

pub const MAX_TIMERS: usize = 50;
pub const MIN_DURATION_MS: u64 = 1_000;
/// 366 days.
pub const MAX_DURATION_MS: u64 = 366 * 24 * 3600 * 1000;
pub const LABEL_MAX: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum State {
    Idle,
    #[serde(rename_all = "camelCase")]
    Running { ends_at: u64 },
    #[serde(rename_all = "camelCase")]
    Paused { remaining_ms: u64 },
    /// `at` is when it was due, which may be earlier than when we noticed (PC asleep, VEX closed).
    #[serde(rename_all = "camelCase")]
    Finished { at: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    pub id: u64,
    /// The widget kind that created it.
    pub owner: String,
    pub label: String,
    pub duration_ms: u64,
    #[serde(flatten)]
    pub state: State,
}

/// Reported exactly once when a running timer reaches its end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub id: u64,
    pub owner: String,
    pub label: String,
    /// How long after its due time we noticed. Large values mean VEX was closed or the PC was asleep.
    pub late_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Timers {
    timers: Vec<Timer>,
    next_id: u64,
    #[serde(skip)]
    fresh: Vec<Finished>,
}

fn clean_label(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(LABEL_MAX).collect()
}

fn check_duration(ms: u64) -> Result<(), String> {
    if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&ms) {
        return Err("A timer must last between 1 second and 366 days.".into());
    }
    Ok(())
}

impl Timers {
    /// Repair anything odd in a loaded file.
    pub fn sanitize(&mut self) {
        let mut seen = std::collections::BTreeSet::new();
        self.timers.retain(|t| seen.insert(t.id) && check_duration(t.duration_ms).is_ok());
        self.timers.truncate(MAX_TIMERS);
        for t in &mut self.timers {
            t.label = clean_label(&t.label);
            if let State::Paused { remaining_ms } = &mut t.state {
                *remaining_ms = (*remaining_ms).min(t.duration_ms);
            }
        }
        let max = self.timers.iter().map(|t| t.id).max().unwrap_or(0);
        self.next_id = self.next_id.max(max + 1).max(1);
    }

    pub fn list(&self) -> &[Timer] {
        &self.timers
    }

    pub fn get(&self, id: u64) -> Option<&Timer> {
        self.timers.iter().find(|t| t.id == id)
    }

    fn find(&mut self, id: u64) -> Result<&mut Timer, String> {
        self.timers.iter_mut().find(|t| t.id == id).ok_or_else(|| "That timer no longer exists.".to_string())
    }

    /// Move every running timer whose time has come to `Finished`, remembering to report it.
    fn expire(&mut self, now: u64) {
        for t in &mut self.timers {
            if let State::Running { ends_at } = t.state {
                if ends_at <= now {
                    t.state = State::Finished { at: ends_at };
                    self.fresh.push(Finished { id: t.id, owner: t.owner.clone(), label: t.label.clone(), late_ms: now - ends_at });
                }
            }
        }
    }

    /// Timers that finished since the last call (each is returned once, in the order they were due).
    pub fn poll(&mut self, now: u64) -> Vec<Finished> {
        self.expire(now);
        std::mem::take(&mut self.fresh)
    }

    /// The earliest moment a running timer ends, or `None` when nothing is running (the scheduler may then
    /// sleep until an operation wakes it).
    pub fn next_deadline(&self) -> Option<u64> {
        self.timers
            .iter()
            .filter_map(|t| match t.state {
                State::Running { ends_at } => Some(ends_at),
                _ => None,
            })
            .min()
    }

    pub fn create(&mut self, owner: &str, label: &str, duration_ms: u64) -> Result<u64, String> {
        check_duration(duration_ms)?;
        if self.timers.len() >= MAX_TIMERS {
            return Err(format!("There is room for {MAX_TIMERS} timers. Delete one first."));
        }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.timers.push(Timer { id, owner: owner.to_string(), label: clean_label(label), duration_ms, state: State::Idle });
        Ok(id)
    }

    /// Start an idle or finished timer from its full duration, or resume a paused one. Running: no change.
    pub fn start(&mut self, id: u64, now: u64) -> Result<(), String> {
        self.expire(now);
        let t = self.find(id)?;
        t.state = match t.state {
            State::Running { .. } => return Ok(()),
            State::Paused { remaining_ms } => State::Running { ends_at: now + remaining_ms },
            State::Idle | State::Finished { .. } => State::Running { ends_at: now + t.duration_ms },
        };
        Ok(())
    }

    pub fn pause(&mut self, id: u64, now: u64) -> Result<(), String> {
        self.expire(now);
        let t = self.find(id)?;
        if let State::Running { ends_at } = t.state {
            t.state = State::Paused { remaining_ms: ends_at.saturating_sub(now) };
        }
        Ok(())
    }

    /// Back to the full duration, stopped.
    pub fn reset(&mut self, id: u64, now: u64) -> Result<(), String> {
        self.expire(now);
        self.find(id)?.state = State::Idle;
        Ok(())
    }

    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id);
        self.fresh.retain(|f| f.id != id);
        if self.timers.len() == before {
            return Err("That timer no longer exists.".into());
        }
        Ok(())
    }

    /// Change the length of a stopped timer (it returns to idle). A running timer must be paused or reset first.
    pub fn set_duration(&mut self, id: u64, duration_ms: u64, now: u64) -> Result<(), String> {
        check_duration(duration_ms)?;
        self.expire(now);
        let t = self.find(id)?;
        if matches!(t.state, State::Running { .. }) {
            return Err("Pause or reset the timer before changing its length.".into());
        }
        t.duration_ms = duration_ms;
        t.state = State::Idle;
        Ok(())
    }

    pub fn set_label(&mut self, id: u64, label: &str) -> Result<(), String> {
        self.find(id)?.label = clean_label(label);
        Ok(())
    }
}

/// Milliseconds left on a timer at `now`.
pub fn remaining_ms(t: &Timer, now: u64) -> u64 {
    match t.state {
        State::Idle => t.duration_ms,
        State::Running { ends_at } => ends_at.saturating_sub(now),
        State::Paused { remaining_ms } => remaining_ms,
        State::Finished { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_700_000_000_000;

    fn one(d: u64) -> (Timers, u64) {
        let mut ts = Timers::default();
        let id = ts.create("pomodoro", "Focus", d).unwrap();
        (ts, id)
    }

    #[test]
    fn runs_to_the_end_and_reports_exactly_once() {
        let (mut ts, id) = one(60_000);
        ts.start(id, T0).unwrap();
        assert_eq!(remaining_ms(ts.get(id).unwrap(), T0 + 10_000), 50_000);
        assert!(ts.poll(T0 + 59_999).is_empty());
        let f = ts.poll(T0 + 60_000);
        assert_eq!(f, vec![Finished { id, owner: "pomodoro".into(), label: "Focus".into(), late_ms: 0 }]);
        assert!(ts.poll(T0 + 90_000).is_empty(), "reported once only");
        assert_eq!(ts.get(id).unwrap().state, State::Finished { at: T0 + 60_000 });
        assert_eq!(remaining_ms(ts.get(id).unwrap(), T0 + 90_000), 0);
    }

    #[test]
    fn remaining_time_is_wall_clock_so_a_sleeping_pc_does_not_lose_time() {
        let (mut ts, id) = one(25 * 60_000);
        ts.start(id, T0).unwrap();
        // No polling at all for 10 minutes (suspended): the answer is still exact.
        assert_eq!(remaining_ms(ts.get(id).unwrap(), T0 + 10 * 60_000), 15 * 60_000);
        // Asleep past the end: noticed late, and we say by how much.
        let f = ts.poll(T0 + 40 * 60_000);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].late_ms, 15 * 60_000);
    }

    #[test]
    fn pause_and_resume_keep_the_remaining_time() {
        let (mut ts, id) = one(60_000);
        ts.start(id, T0).unwrap();
        ts.pause(id, T0 + 20_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Paused { remaining_ms: 40_000 });
        assert_eq!(ts.next_deadline(), None);
        // A long pause does not eat into it.
        ts.start(id, T0 + 3_600_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Running { ends_at: T0 + 3_600_000 + 40_000 });
        assert!(ts.poll(T0 + 3_600_000 + 39_999).is_empty());
        assert_eq!(ts.poll(T0 + 3_600_000 + 40_000).len(), 1);
    }

    #[test]
    fn pausing_after_the_end_finishes_instead_of_going_negative() {
        let (mut ts, id) = one(10_000);
        ts.start(id, T0).unwrap();
        ts.pause(id, T0 + 20_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Finished { at: T0 + 10_000 });
        assert_eq!(ts.poll(T0 + 20_000).len(), 1, "the alert is not lost");
    }

    #[test]
    fn reset_start_again_and_start_is_idempotent() {
        let (mut ts, id) = one(10_000);
        ts.start(id, T0).unwrap();
        ts.start(id, T0 + 5_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Running { ends_at: T0 + 10_000 }, "a second start does not restart");
        ts.reset(id, T0 + 6_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Idle);
        assert_eq!(ts.next_deadline(), None);
        ts.start(id, T0 + 7_000).unwrap();
        ts.poll(T0 + 17_000);
        ts.start(id, T0 + 18_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Running { ends_at: T0 + 28_000 }, "a finished timer restarts from its full length");
    }

    #[test]
    fn deadline_is_the_earliest_running_timer() {
        let mut ts = Timers::default();
        let a = ts.create("x", "a", 30_000).unwrap();
        let b = ts.create("x", "b", 10_000).unwrap();
        let _c = ts.create("x", "c", 5_000).unwrap();
        assert_eq!(ts.next_deadline(), None);
        ts.start(a, T0).unwrap();
        ts.start(b, T0).unwrap();
        assert_eq!(ts.next_deadline(), Some(T0 + 10_000));
        let f = ts.poll(T0 + 12_000);
        assert_eq!(f.iter().map(|f| f.id).collect::<Vec<_>>(), vec![b]);
        assert_eq!(ts.next_deadline(), Some(T0 + 30_000));
    }

    #[test]
    fn simultaneous_finishes_are_all_reported() {
        let mut ts = Timers::default();
        let ids: Vec<_> = (0..3).map(|i| ts.create("x", &format!("t{i}"), 5_000).unwrap()).collect();
        for id in &ids {
            ts.start(*id, T0).unwrap();
        }
        assert_eq!(ts.poll(T0 + 5_000).len(), 3);
    }

    #[test]
    fn restart_while_running_keeps_the_timer_and_reports_one_that_ended_while_closed() {
        let mut ts = Timers::default();
        let long = ts.create("x", "long", 3_600_000).unwrap();
        let short = ts.create("x", "short", 60_000).unwrap();
        ts.start(long, T0).unwrap();
        ts.start(short, T0).unwrap();
        let saved = serde_json::to_string(&ts).unwrap();

        // VEX was closed for ten minutes.
        let mut loaded: Timers = serde_json::from_str(&saved).unwrap();
        loaded.sanitize();
        let f = loaded.poll(T0 + 600_000);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].id, f[0].late_ms), (short, 540_000));
        assert_eq!(remaining_ms(loaded.get(long).unwrap(), T0 + 600_000), 3_000_000);
        assert_eq!(loaded.next_deadline(), Some(T0 + 3_600_000));
    }

    #[test]
    fn json_shape_for_the_ui() {
        let (mut ts, id) = one(60_000);
        ts.start(id, T0).unwrap();
        let v = serde_json::to_value(&ts.list()[0]).unwrap();
        assert_eq!(v, serde_json::json!({"id":id,"owner":"pomodoro","label":"Focus","durationMs":60000,"state":"running","endsAt":T0+60_000}));
        ts.pause(id, T0 + 10_000).unwrap();
        let v = serde_json::to_value(&ts.list()[0]).unwrap();
        assert_eq!(v["state"], "paused");
        assert_eq!(v["remainingMs"], 50_000);
    }

    #[test]
    fn limits_and_validation() {
        let mut ts = Timers::default();
        assert!(ts.create("x", "", 999).is_err());
        assert!(ts.create("x", "", MAX_DURATION_MS + 1).is_err());
        assert!(ts.create("x", "", MAX_DURATION_MS).is_ok());
        let id = ts.create("x", "  Tea\u{0}\n time  ", 1_000).unwrap();
        assert_eq!(ts.get(id).unwrap().label, "Tea time");
        let long = "é".repeat(500);
        let id2 = ts.create("x", &long, 5_000).unwrap();
        assert_eq!(ts.get(id2).unwrap().label.chars().count(), LABEL_MAX);
        while ts.list().len() < MAX_TIMERS {
            ts.create("x", "", 5_000).unwrap();
        }
        assert!(ts.create("x", "", 5_000).unwrap_err().contains("room"));
    }

    #[test]
    fn set_duration_only_when_stopped_and_unknown_ids_fail_cleanly() {
        let (mut ts, id) = one(60_000);
        ts.start(id, T0).unwrap();
        assert!(ts.set_duration(id, 5_000, T0).unwrap_err().contains("Pause or reset"));
        ts.pause(id, T0 + 1_000).unwrap();
        ts.set_duration(id, 5_000, T0 + 2_000).unwrap();
        assert_eq!(ts.get(id).unwrap().state, State::Idle);
        assert_eq!(ts.get(id).unwrap().duration_ms, 5_000);
        assert!(ts.set_duration(id, 10, T0).is_err());
        for r in [ts.start(999, T0), ts.pause(999, T0), ts.reset(999, T0), ts.delete(999), ts.set_label(999, "x")] {
            assert!(r.unwrap_err().contains("no longer exists"));
        }
    }

    #[test]
    fn deleting_a_timer_drops_its_pending_alert_and_ids_are_not_reused() {
        let (mut ts, id) = one(5_000);
        ts.start(id, T0).unwrap();
        ts.pause(id, T0 + 6_000).unwrap(); // finishes, alert pending
        ts.delete(id).unwrap();
        assert!(ts.poll(T0 + 7_000).is_empty());
        assert!(ts.create("x", "", 5_000).unwrap() > id);
    }

    #[test]
    fn sanitize_repairs_a_damaged_file() {
        let mut ts: Timers = serde_json::from_str(
            r#"{"timers":[
                {"id":4,"owner":"x","label":"ok","durationMs":5000,"state":"idle"},
                {"id":4,"owner":"x","label":"dup","durationMs":5000,"state":"idle"},
                {"id":5,"owner":"x","label":"zero","durationMs":0,"state":"idle"},
                {"id":6,"owner":"x","label":"p","durationMs":5000,"state":"paused","remainingMs":99999}
            ],"nextId":1}"#,
        )
        .unwrap();
        ts.sanitize();
        assert_eq!(ts.list().iter().map(|t| t.id).collect::<Vec<_>>(), vec![4, 6]);
        assert_eq!(ts.get(6).unwrap().state, State::Paused { remaining_ms: 5_000 });
        assert!(ts.create("x", "", 5_000).unwrap() >= 7);
    }
}

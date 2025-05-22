use std::sync::{Arc, Mutex};
use std::time::Duration;

use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use tauri::{AppHandle, Emitter};
use tokio::time;
use tokio::sync::Mutex as TokioMutex;

use crate::user::increment_pomodoros_done;

#[derive(Clone, serde::Serialize)]
pub struct TimerUpdatePayload {
    pub state: String,
    pub remaining_time: u64,
    pub interval_time: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimerState {
    Idle,
    Running,
    Paused,
    ShortBreak,
    LongBreak,
}

pub struct PomodoroTimer {
    // runtime state
    state: Arc<Mutex<TimerState>>,                
    last_active_state: Arc<Mutex<Option<TimerState>>>, 

    remaining_seconds: Arc<Mutex<u64>>,           
    current_session: Arc<Mutex<u32>>,             

    // constants 
    work_duration: Duration,
    short_break_duration: Duration,
    long_break_duration: Duration,
    sessions_before_long_break: u32,
    interval_time: u64,                           

    app_handle: AppHandle,
}

impl PomodoroTimer {
    pub fn new(app_handle: AppHandle) -> Self {
        let work_secs = 5; 
        let short_break_secs = 5; 
        let long_break_secs = 15 * 60; 
        let interval_time = 1;

        Self {
            state: Arc::new(Mutex::new(TimerState::Idle)),
            last_active_state: Arc::new(Mutex::new(None)),

            remaining_seconds: Arc::new(Mutex::new(work_secs)),
            current_session: Arc::new(Mutex::new(1)),

            work_duration: Duration::from_secs(work_secs),
            short_break_duration: Duration::from_secs(short_break_secs),
            long_break_duration: Duration::from_secs(long_break_secs),
            sessions_before_long_break: 4,
            interval_time,

            app_handle,
        }
    }

    // ───────────────────────── public api ────────────────────────────────

    pub async fn start(&self) -> Result<(), &'static str> {
        if *self.state.lock().unwrap() != TimerState::Idle {
            return Err("timer already started; use resume()");
        }
        self.boot_cycle(TimerState::Running, self.work_duration.as_secs())
            .await;
        Ok(())
    }

    pub async fn resume(&self) -> Result<(), &'static str> {
        if *self.state.lock().unwrap() != TimerState::Paused {
            return Err("timer is not paused");
        }
        let prev = self
            .last_active_state
            .lock()
            .unwrap()
            .take()
            .unwrap_or(TimerState::Running);
        let secs_left = *self.remaining_seconds.lock().unwrap();
        self.boot_cycle(prev, secs_left).await;
        Ok(())
    }

    pub fn pause(&self) {
        let mut st = self.state.lock().unwrap();
        if matches!(*st, TimerState::Running | TimerState::ShortBreak | TimerState::LongBreak) {
            *self.last_active_state.lock().unwrap() = Some(*st);
            *st = TimerState::Paused;
        }
    }

    pub fn stop(&self) {
        *self.state.lock().unwrap() = TimerState::Idle;
        *self.remaining_seconds.lock().unwrap() = self.work_duration.as_secs();
        *self.current_session.lock().unwrap() = 1;
        let _ = self.app_handle.emit(
            "timer_updated",
            TimerUpdatePayload {
                state: "idle".into(),
                remaining_time: self.work_duration.as_secs(),
                interval_time: 0,
            },
        );
    }

    // ─────────────────────── internal helpers ────────────────────────────

    async fn boot_cycle(&self, new_state: TimerState, initial_secs: u64) {
        *self.state.lock().unwrap() = new_state;
        *self.remaining_seconds.lock().unwrap() = initial_secs;

        static LOOP_STARTED: OnceCell<()> = OnceCell::new();
        if LOOP_STARTED.set(()).is_ok() {
            self.spawn_loop();
        }
    }

    fn spawn_loop(&self) {
        let st = Arc::clone(&self.state);
        let remain = Arc::clone(&self.remaining_seconds);
        let session = Arc::clone(&self.current_session);
        let app = self.app_handle.clone();
        let sbreak_secs = self.short_break_duration.as_secs();
        let lbreak_secs = self.long_break_duration.as_secs();
        let work_secs = self.work_duration.as_secs();
        let every = self.sessions_before_long_break;
        let tick = self.interval_time; 

        tauri::async_runtime::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(tick)).await;

                // skip ticking while paused
                if *st.lock().unwrap() == TimerState::Paused {
                    continue;
                }
                if *st.lock().unwrap() != TimerState::Idle {
                // tick or transition
                let mut rem = remain.lock().unwrap();
                if *rem > tick {
                    *rem -= tick;
                } else {
                    let mut s = st.lock().unwrap();
                    let mut sess = session.lock().unwrap();
                    match *s {
                        TimerState::Running => {
                            if *sess % every == 0 {
                                *s = TimerState::LongBreak;
                                *rem = lbreak_secs;
                            } else {
                                *s = TimerState::ShortBreak;
                                *rem = sbreak_secs;
                            }
                            *sess += 1;
                            increment_pomodoros_done(app.clone());
                        }
                        TimerState::ShortBreak | TimerState::LongBreak => {
                            *s = TimerState::Running;
                            *rem = work_secs;
                        }
                        _ => {}
                    }
                }

                // broadcast update
                let _ = app.emit(
                    "timer_updated",
                    TimerUpdatePayload {
                        state: format!("{:?}", *st.lock().unwrap()).to_lowercase(),
                        remaining_time: *rem,
                        interval_time: tick,
                    },
                );
            }
            }
        });
    }
}

// ───────────────────────── module globals ────────────────────────────────

lazy_static! {
    static ref POMODORO: TokioMutex<Option<PomodoroTimer>> = TokioMutex::new(None);
}

pub async fn init_pomodoro(app: AppHandle) { 
    let mut guard = POMODORO.lock().await;
    *guard = Some(PomodoroTimer::new(app));
}

// ─────────────────────────── tauri commands ──────────────────────────────

#[tauri::command]
pub async fn start_timer() -> Result<(), String> {
    let guard = POMODORO.lock().await; // Async lock
    if let Some(timer) = guard.as_ref() {
        timer.start().await.map_err(|e| e.to_string()) // timer.start() is already async
    } else {
        Err("pomodoro timer not initialized".to_string())
    }
}

#[tauri::command]
pub async fn resume_timer() -> Result<(), String> { 
    let guard = POMODORO.lock().await; 
    if let Some(timer) = guard.as_ref() {
        timer.resume().await.map_err(|e| e.to_string())
    } else {
        Err("pomodoro timer not initialized".to_string())
    }
}

#[tauri::command]
pub async fn pause_timer() -> Result<(), String> {
    let guard = POMODORO.lock().await;
    if let Some(timer) = guard.as_ref() {
        timer.pause(); 
        Ok(())
    } else {
        Err("pomodoro timer not initialized".to_string())
    }
}

#[tauri::command]
pub async fn stop_time() -> Result<(), String> {
    let guard = POMODORO.lock().await;
    if let Some(timer) = guard.as_ref() {
        timer.stop();
        Ok(())
    } else {
        Err("pomodoro timer not initialized".to_string())
    }
}

// ───────────── convenience wrappers for invoke() callers ────────────────

pub async fn command_start_pomodoro() -> Result<String, String> {
    start_timer().await.map(|_| "pomodoro started".into())
}

pub async fn command_resume_pomodoro() -> Result<String, String> {
    resume_timer().await.map(|_| "pomodoro resumed".into())
}

pub async fn command_pause_pomodoro() -> Result<String, String> {
    pause_timer().await.map(|_| "pomodoro paused".into())
}

pub async fn command_stop_pomodoro() -> Result<String, String> {
    stop_time().await.map(|_| "pomodoro stopped".into())
}

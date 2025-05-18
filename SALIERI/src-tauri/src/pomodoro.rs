use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::interval;
use lazy_static::lazy_static;

#[derive(Clone, serde::Serialize)]
pub struct TimerUpdatePayload {
    pub state: String,
    pub remaining_time: u64,
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
    state: Arc<Mutex<TimerState>>,
    remaining_seconds: Arc<Mutex<u64>>,
    work_duration: Duration,
    short_break_duration: Duration,
    long_break_duration: Duration,
    sessions_before_long_break: u32,
    current_session: Arc<Mutex<u32>>,
    interval: Arc<Mutex<Option<tokio::time::Interval>>>,
    app_handle: AppHandle,
}

impl PomodoroTimer {
    fn new(app_handle: AppHandle) -> Self {
        let initial_work_duration_secs = 25 * 60;
        PomodoroTimer {
            state: Arc::new(Mutex::new(TimerState::Idle)),
            remaining_seconds: Arc::new(Mutex::new(initial_work_duration_secs)),
            work_duration: Duration::from_secs(initial_work_duration_secs),
            short_break_duration: Duration::from_secs(60 * 5),
            long_break_duration: Duration::from_secs(15 * 5),
            sessions_before_long_break: 4,
            current_session: Arc::new(Mutex::new(1)),
            interval: Arc::new(Mutex::new(None)),
            app_handle,
        }
    }

    pub async fn start(&self) {
        let mut oldPaused = false;
        let mut state = self.state.lock().unwrap();
        if matches!(*state, TimerState::Running) {
            return;
        }
        let mut resume_from = None;
        if matches!(*state, TimerState::Paused){
            oldPaused = true;
            resume_from = Some(*self.remaining_seconds.lock().unwrap());
        }
        let initial_seconds = match resume_from {
            Some(secs) => secs,
            None => self.work_duration.as_secs(),
        };
        *state = TimerState::Running;
        *self.remaining_seconds.lock().unwrap() = initial_seconds;

        let app_handle = self.app_handle.clone();
        let remaining_seconds = Arc::clone(&self.remaining_seconds);
        let state_clone = Arc::clone(&self.state);
        let next_session = Arc::clone(&self.current_session);
        let long_break_interval = self.sessions_before_long_break;
        let short_break_duration = self.short_break_duration;
        let long_break_duration = self.long_break_duration;
        let work_duration = self.work_duration;
        let interval_clone = Arc::clone(&self.interval);
        
        let interval = interval(Duration::from_secs(1));
        *self.interval.lock().unwrap() = Some(interval);
        tokio::spawn(async move {
            if oldPaused != true {
                loop {
                    let mut __internal_should_sleep_and_continue = false;
                    {
                        let current_interval_guard = interval_clone.lock().unwrap();
                        if current_interval_guard.is_none(){
                            let __internal_state_guard = state_clone.lock().unwrap();
                            if *__internal_state_guard == TimerState::Paused {
                                __internal_should_sleep_and_continue = true;
                            }
                        }
                    }
                    if __internal_should_sleep_and_continue {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        continue;
                    }

                    {
                        let mut remaining = remaining_seconds.lock().unwrap();
                        if *remaining > 0 {
                            *remaining -= 1;
                        } else {
                            let current_state = *state_clone.lock().unwrap();
                            let mut session = next_session.lock().unwrap();
                            match current_state {
                                TimerState::Running => {
                                    if *session % long_break_interval == 0 {
                                        *state_clone.lock().unwrap() = TimerState::LongBreak;
                                        *remaining = long_break_duration.as_secs();
                                    } else {
                                        *state_clone.lock().unwrap() = TimerState::ShortBreak;
                                        *remaining = short_break_duration.as_secs();
                                    }
                                    *session += 1;
                                }
                                TimerState::ShortBreak | TimerState::LongBreak => {
                                    *state_clone.lock().unwrap() = TimerState::Running;
                                    *remaining = work_duration.as_secs();
                                }
                                _ => break,
                            }
                        }
                        let current_state = *state_clone.lock().unwrap();
                        let _ = app_handle.emit(
                            "timer_updated",
                            TimerUpdatePayload {
                                state: format!("{:?}", current_state).to_lowercase(),
                                remaining_time: *remaining,
                            },
                        );
                    }
                    // ← THIS LINE RESTORED
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        });
    }

    pub fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if matches!(*state, TimerState::Running) {
            *state = TimerState::Paused;
            *self.interval.lock().unwrap() = None; 
        }
    }

    pub fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        *state = TimerState::Idle; 

        let mut remaining_seconds = self.remaining_seconds.lock().unwrap();
        *remaining_seconds = 0; 

        let mut current_session = self.current_session.lock().unwrap();
        *current_session = 1; 

        *self.interval.lock().unwrap() = None; 

        let _ = self.app_handle.emit(
            "timer_updated",
            TimerUpdatePayload { state: "idle".into(), remaining_time: 0 },
        );
    }
}

lazy_static! {
    static ref POMODORO: Mutex<Option<PomodoroTimer>> = Mutex::new(None);
}

pub fn init_pomodoro(app_handle: AppHandle) {
    let mut timer = POMODORO.lock().unwrap();
    *timer = Some(PomodoroTimer::new(app_handle));
}

#[tauri::command]
pub async fn start_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.start().await;
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

#[tauri::command]
pub fn pause_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.pause();
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

#[tauri::command]
pub fn stop_timer() -> Result<(), String> {
    let timer_guard = POMODORO.lock().unwrap();
    if let Some(ref timer) = *timer_guard {
        timer.stop();
        Ok(())
    } else {
        Err("Pomodoro timer not initialized".into())
    }
}

pub fn command_start_pomodoro() -> Result<String, String> {
    tauri::async_runtime::block_on(start_timer()).map(|_| "pomodoro started".into())
}

pub fn command_pause_pomodoro() -> Result<String, String> {
    pause_timer().map(|_| "pomodoro paused".into())
}

pub fn command_stop_pomodoro() -> Result<String, String> {
    stop_timer().map(|_| "pomodoro stopped".into())
}
use rdev::{listen, Button, Event, EventType, Key};
use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

const JOYSTICK_X: i32 = 468;
const JOYSTICK_Y: i32 = 789;
const FIRE_X: i32 = 1942;
const FIRE_Y: i32 = 540;
const SPRINT_X: i32 = 2012;
const SPRINT_Y: i32 = 702;
const CROUCH_X: i32 = 2141;
const CROUCH_Y: i32 = 886;
const RELOAD_X: i32 = 1850;
const RELOAD_Y: i32 = 880;
const CAMERA_START_X: i32 = 1600;
const CAMERA_START_Y: i32 = 540;
const CAMERA_SENSITIVITY: f64 = 2.0;

static INPUT_ENABLED: AtomicBool = AtomicBool::new(false);
static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);
static MOVE_W: AtomicBool = AtomicBool::new(false);
static MOVE_S: AtomicBool = AtomicBool::new(false);
static MOVE_A: AtomicBool = AtomicBool::new(false);
static MOVE_D: AtomicBool = AtomicBool::new(false);
static MOUSE_DX: AtomicI64 = AtomicI64::new(0);
static MOUSE_DY: AtomicI64 = AtomicI64::new(0);
static LAST_MOUSE_POSITION: Mutex<Option<(f64, f64)>> = Mutex::new(None);
static ACTIVE_SERIAL: Mutex<Option<String>> = Mutex::new(None);

pub fn start_input_listener(serial: String) -> Result<(), String> {
    {
        let mut active_serial = ACTIVE_SERIAL
            .lock()
            .map_err(|_| "failed to lock input serial".to_string())?;
        *active_serial = Some(serial);
    }

    INPUT_ENABLED.store(true, Ordering::SeqCst);
    reset_input_state();

    if !LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        spawn_movement_thread();
        spawn_mouse_thread();
        spawn_listener_thread();
    }

    Ok(())
}

pub fn stop_input_listener() {
    INPUT_ENABLED.store(false, Ordering::SeqCst);
    reset_input_state();

    if let Ok(mut active_serial) = ACTIVE_SERIAL.lock() {
        *active_serial = None;
    }
}

fn spawn_movement_thread() {
    thread::spawn(|| loop {
        if INPUT_ENABLED.load(Ordering::SeqCst) {
            if MOVE_W.load(Ordering::SeqCst) {
                adb_swipe(JOYSTICK_X, JOYSTICK_Y, JOYSTICK_X, JOYSTICK_Y - 100, 50);
            }
            if MOVE_S.load(Ordering::SeqCst) {
                adb_swipe(JOYSTICK_X, JOYSTICK_Y, JOYSTICK_X, JOYSTICK_Y + 100, 50);
            }
            if MOVE_A.load(Ordering::SeqCst) {
                adb_swipe(JOYSTICK_X, JOYSTICK_Y, JOYSTICK_X - 100, JOYSTICK_Y, 50);
            }
            if MOVE_D.load(Ordering::SeqCst) {
                adb_swipe(JOYSTICK_X, JOYSTICK_Y, JOYSTICK_X + 100, JOYSTICK_Y, 50);
            }
        }

        thread::sleep(Duration::from_millis(50));
    });
}

fn spawn_mouse_thread() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_millis(16));

        if !INPUT_ENABLED.load(Ordering::SeqCst) {
            MOUSE_DX.store(0, Ordering::SeqCst);
            MOUSE_DY.store(0, Ordering::SeqCst);
            continue;
        }

        let dx = MOUSE_DX.swap(0, Ordering::SeqCst);
        let dy = MOUSE_DY.swap(0, Ordering::SeqCst);

        if dx == 0 && dy == 0 {
            continue;
        }

        let end_x = CAMERA_START_X + ((dx as f64) * CAMERA_SENSITIVITY) as i32;
        let end_y = CAMERA_START_Y + ((dy as f64) * CAMERA_SENSITIVITY) as i32;
        adb_swipe(CAMERA_START_X, CAMERA_START_Y, end_x, end_y, 16);
    });
}

fn spawn_listener_thread() {
    thread::spawn(|| {
        let callback = |event: Event| {
            handle_event(event);
        };

        if let Err(error) = listen(callback) {
            eprintln!("global input listener failed: {error:?}");
        }

        LISTENER_RUNNING.store(false, Ordering::SeqCst);
        INPUT_ENABLED.store(false, Ordering::SeqCst);
    });
}

fn handle_event(event: Event) {
    match event.event_type {
        EventType::KeyPress(key) => handle_key_press(key),
        EventType::KeyRelease(key) => handle_key_release(key),
        EventType::ButtonPress(button) => handle_button_press(button),
        EventType::MouseMove { x, y } => handle_mouse_move(x, y),
        _ => {}
    }
}

fn handle_key_press(key: Key) {
    if key == Key::F1 {
        let enabled = !INPUT_ENABLED.load(Ordering::SeqCst);
        INPUT_ENABLED.store(enabled, Ordering::SeqCst);
        reset_input_state();
        return;
    }

    if !INPUT_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    match key {
        Key::KeyW => MOVE_W.store(true, Ordering::SeqCst),
        Key::KeyS => MOVE_S.store(true, Ordering::SeqCst),
        Key::KeyA => MOVE_A.store(true, Ordering::SeqCst),
        Key::KeyD => MOVE_D.store(true, Ordering::SeqCst),
        Key::KeyR => adb_tap(RELOAD_X, RELOAD_Y),
        Key::ShiftLeft | Key::ShiftRight => adb_tap(SPRINT_X, SPRINT_Y),
        Key::ControlLeft | Key::ControlRight => adb_tap(CROUCH_X, CROUCH_Y),
        _ => {}
    }
}

fn handle_key_release(key: Key) {
    if !INPUT_ENABLED.load(Ordering::SeqCst) {
        reset_movement_key(key);
        return;
    }

    match key {
        Key::KeyW | Key::KeyS | Key::KeyA | Key::KeyD => {
            reset_movement_key(key);
            adb_tap(JOYSTICK_X, JOYSTICK_Y);
        }
        _ => {}
    }
}

fn handle_button_press(button: Button) {
    if !INPUT_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    match button {
        Button::Left | Button::Right => adb_tap(FIRE_X, FIRE_Y),
        _ => {}
    }
}

fn handle_mouse_move(x: f64, y: f64) {
    let Ok(mut last_position) = LAST_MOUSE_POSITION.lock() else {
        return;
    };

    if let Some((last_x, last_y)) = *last_position {
        if INPUT_ENABLED.load(Ordering::SeqCst) {
            MOUSE_DX.fetch_add((x - last_x) as i64, Ordering::SeqCst);
            MOUSE_DY.fetch_add((y - last_y) as i64, Ordering::SeqCst);
        }
    }

    *last_position = Some((x, y));
}

fn reset_movement_key(key: Key) {
    match key {
        Key::KeyW => MOVE_W.store(false, Ordering::SeqCst),
        Key::KeyS => MOVE_S.store(false, Ordering::SeqCst),
        Key::KeyA => MOVE_A.store(false, Ordering::SeqCst),
        Key::KeyD => MOVE_D.store(false, Ordering::SeqCst),
        _ => {}
    }
}

fn reset_input_state() {
    MOVE_W.store(false, Ordering::SeqCst);
    MOVE_S.store(false, Ordering::SeqCst);
    MOVE_A.store(false, Ordering::SeqCst);
    MOVE_D.store(false, Ordering::SeqCst);
    MOUSE_DX.store(0, Ordering::SeqCst);
    MOUSE_DY.store(0, Ordering::SeqCst);

    if let Ok(mut last_position) = LAST_MOUSE_POSITION.lock() {
        *last_position = None;
    }
}

fn adb_tap(x: i32, y: i32) {
    run_adb(["shell", "input", "tap", &x.to_string(), &y.to_string()]);
}

fn adb_swipe(start_x: i32, start_y: i32, end_x: i32, end_y: i32, duration_ms: i32) {
    run_adb([
        "shell",
        "input",
        "swipe",
        &start_x.to_string(),
        &start_y.to_string(),
        &end_x.to_string(),
        &end_y.to_string(),
        &duration_ms.to_string(),
    ]);
}

fn run_adb<const N: usize>(args: [&str; N]) {
    let serial = match ACTIVE_SERIAL.lock() {
        Ok(active_serial) => active_serial.clone(),
        Err(_) => None,
    };

    let Some(serial) = serial else {
        return;
    };

    let _ = Command::new("adb")
        .arg("-s")
        .arg(serial)
        .args(args)
        .status();
}

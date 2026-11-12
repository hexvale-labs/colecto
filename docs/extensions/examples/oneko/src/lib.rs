#![no_std]
use core::fmt::Write;
use serde::Deserialize;
#[link(wasm_import_module = "colecto")]
unsafe extern "C" {
    fn request(ptr: *const u8, len: usize, output: *mut u8, capacity: usize) -> i32;
    fn action(ptr: *mut u8, capacity: usize) -> i32;
    fn emit(ptr: *const u8, len: usize) -> i32;
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
#[derive(Deserialize)]
struct Pointer {
    x: f32,
    y: f32,
    inside: bool,
}
#[derive(Deserialize)]
struct Viewport {
    width: f32,
    height: f32,
}
#[derive(Deserialize)]
struct Input<'a> {
    pointer: Pointer,
    viewport: Viewport,
    state: &'a str,
}
#[derive(Deserialize)]
struct Settings { speed: f32, opacity: f32, stop_distance: f32 }
#[derive(Deserialize)]
struct Response { data: Settings }
#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let mut buffer = [0u8; 8192];
    let len = unsafe { action(buffer.as_mut_ptr(), buffer.len()) };
    if len < 0 {
        return;
    }
    let Ok((input, _)) = serde_json_core::from_slice::<Input>(&buffer[..len as usize]) else {
        return;
    };
    let query = br#"{"operation":"settings.read"}"#;
    let mut response = [0u8; 1024];
    let size = unsafe { request(query.as_ptr(), query.len(), response.as_mut_ptr(), response.len()) };
    let settings = if size > 0 { serde_json_core::from_slice::<Response>(&response[..size as usize]).ok().map(|(r, _)| r.data) } else { None };
    let settings = settings.unwrap_or(Settings { speed: 10.0, opacity: 100.0, stop_distance: 44.0 });
    let opacity = settings.opacity / 100.0;
    let mut values = input.state.split(',');
    let mut number = || {
        values
            .next()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let (mut x, mut y, mut idle, mut tick) = (number(), number(), number(), number());
    tick += 1.0;
    let max_x = (input.viewport.width - 32.0).max(0.0);
    let max_y = (input.viewport.height - 32.0).max(0.0);
    let px = input.pointer.x;
    let py = input.pointer.y;
    let dx = px - (x + 16.0);
    let dy = py - (y + 16.0);
    let distance = dx.abs().max(dy.abs());
    let phase = (tick as usize) % 2;
    let frame;
    if input.pointer.inside && dx.abs() < 22.0 && dy.abs() < 22.0 {
        x = if px + 56.0 <= input.viewport.width {
            px + 24.0
        } else {
            px - 56.0
        };
        y = if py + 56.0 <= input.viewport.height {
            py + 24.0
        } else {
            py - 56.0
        };
        idle = 0.0;
        frame = (7, 3);
    } else if input.pointer.inside && distance > settings.stop_distance {
        if idle > 1.0 {
            idle = idle.min(7.0) - 1.0;
            frame = (7, 3);
        } else {
            x += dx / distance * settings.speed;
            y += dy / distance * settings.speed;
            let horizontal = dx.abs() > distance * 0.5;
            let vertical = dy.abs() > distance * 0.5;
            let frames = match (horizontal, vertical, dx > 0.0, dy > 0.0) {
                (true, true, true, true) => [(5, 1), (5, 2)],
                (true, true, true, false) => [(0, 2), (0, 3)],
                (true, true, false, true) => [(5, 3), (6, 1)],
                (true, true, false, false) => [(1, 0), (1, 1)],
                (true, false, true, _) => [(3, 0), (3, 1)],
                (true, false, false, _) => [(4, 2), (4, 3)],
                (_, _, _, true) => [(6, 3), (7, 2)],
                _ => [(1, 2), (1, 3)],
            };
            frame = frames[phase];
        }
    } else {
        idle += 1.0;
        frame = if idle > 100.0 {
            (2, ((tick as usize / 4) % 2))
        } else if idle > 92.0 {
            (3, 2)
        } else if idle > 40.0 && idle < 52.0 {
            (5 + tick as usize % 3, 0)
        } else {
            (3, 3)
        };
    }
    x = x.clamp(0.0, max_x);
    y = y.clamp(0.0, max_y);
    let mut output = heapless::String::<1024>::new();
    let _ = write!(
        output,
        "{{\"overlay\":{{\"items\":[{{\"image\":\"oneko.png\",\"x\":{x},\"y\":{y},\"width\":32,\"height\":32,\"opacity\":{opacity},\"frame\":{{\"x\":{},\"y\":{},\"width\":32,\"height\":32}}}}]}},\"state\":\"{x},{y},{idle},{tick}\"}}",
        frame.0 * 32,
        frame.1 * 32
    );
    unsafe {
        emit(output.as_ptr(), output.len());
    }
}

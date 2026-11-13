//! A pixel cat that chases the pointer, written against `colecto-extension`.
#![forbid(unsafe_code)]

use colecto_extension::prelude::*;

const SIZE: f64 = 32.0;

/// Owner settings from `extension.toml`.
#[derive(Deserialize)]
struct Settings {
    speed: f64,
    opacity: f64,
    stop_distance: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            speed: 10.0,
            opacity: 100.0,
            stop_distance: 44.0,
        }
    }
}

/// Carried from one frame to the next by the Host.
#[derive(Default, Serialize, Deserialize)]
struct Cat {
    x: f64,
    y: f64,
    idle: f64,
    tick: f64,
}

fn frame(frame: Frame<Cat>, cx: &mut Context) -> Result<Overlay<Cat>> {
    let settings: Settings = cx.settings().unwrap_or_default();
    let Frame {
        pointer,
        viewport,
        state: mut cat,
        ..
    } = frame;
    cat.tick += 1.0;
    let max_x = (viewport.width - SIZE).max(0.0);
    let max_y = (viewport.height - SIZE).max(0.0);
    let (px, py) = (pointer.x, pointer.y);
    let dx = px - (cat.x + SIZE / 2.0);
    let dy = py - (cat.y + SIZE / 2.0);
    let distance = dx.abs().max(dy.abs());
    let phase = (cat.tick as usize) % 2;
    let sprite;
    if pointer.inside && dx.abs() < 22.0 && dy.abs() < 22.0 {
        // The pointer landed on the cat: hop beside it.
        cat.x = if px + 56.0 <= viewport.width {
            px + 24.0
        } else {
            px - 56.0
        };
        cat.y = if py + 56.0 <= viewport.height {
            py + 24.0
        } else {
            py - 56.0
        };
        cat.idle = 0.0;
        sprite = (7, 3);
    } else if pointer.inside && distance > settings.stop_distance {
        if cat.idle > 1.0 {
            // Wake up for a few frames before running.
            cat.idle = cat.idle.min(7.0) - 1.0;
            sprite = (7, 3);
        } else {
            cat.x += dx / distance * settings.speed;
            cat.y += dy / distance * settings.speed;
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
            sprite = frames[phase];
        }
    } else {
        cat.idle += 1.0;
        sprite = if cat.idle > 100.0 {
            (2, (cat.tick as usize / 4) % 2)
        } else if cat.idle > 92.0 {
            (3, 2)
        } else if cat.idle > 40.0 && cat.idle < 52.0 {
            (5 + cat.tick as usize % 3, 0)
        } else {
            (3, 3)
        };
    }
    cat.x = cat.x.clamp(0.0, max_x);
    cat.y = cat.y.clamp(0.0, max_y);
    let crop = Rect::new(sprite.0 as f64 * SIZE, sprite.1 as f64 * SIZE, SIZE, SIZE);
    let item = Sprite::new("oneko.png", crop)
        .at(cat.x, cat.y)
        .opacity(settings.opacity / 100.0);
    Ok(Overlay {
        items: vec![item],
        state: cat,
    })
}

colecto_extension::export! { overlay: frame }

#[cfg(test)]
mod tests {
    use colecto_extension::serde_json::{self, json};
    use colecto_extension::testing::TestHost;

    fn host() -> TestHost {
        TestHost::new(|request| match request["operation"].as_str() {
            Some("settings.read") => Ok(json!({"speed": 10, "opacity": 50, "stop_distance": 44})),
            _ => Err("Capability not granted".into()),
        })
    }

    fn frame(x: f64, y: f64, state: &str) -> String {
        json!({"event": "frame", "time_ms": 0, "pointer": {"x": x, "y": y, "inside": true},
               "viewport": {"width": 800, "height": 600}, "state": state})
        .to_string()
    }

    #[test]
    fn runs_toward_the_pointer_and_keeps_state() {
        let mut host = host();
        let output = host
            .run(super::colecto_main, &frame(400.0, 16.0, ""))
            .unwrap();
        let item = &output["overlay"]["items"][0];
        assert_eq!(
            (item["x"].as_f64(), item["y"].as_f64()),
            (Some(10.0), Some(0.0))
        );
        assert_eq!(item["opacity"], 0.5);
        assert_eq!(
            item["frame"],
            json!({"x": 96.0, "y": 32.0, "width": 32.0, "height": 32.0})
        );
        let state = output["state"].as_str().unwrap();
        let next = host
            .run(super::colecto_main, &frame(400.0, 16.0, state))
            .unwrap();
        assert_eq!(next["overlay"]["items"][0]["x"].as_f64(), Some(20.0));
    }

    #[test]
    fn old_state_strings_start_fresh() {
        // Packages before the SDK stored "x,y,idle,tick"; that no longer parses.
        let output = host()
            .run(super::colecto_main, &frame(400.0, 16.0, "5,5,0,9"))
            .unwrap();
        let state: serde_json::Value =
            serde_json::from_str(output["state"].as_str().unwrap()).unwrap();
        assert_eq!(
            (state["x"].as_f64(), state["tick"].as_f64()),
            (Some(10.0), Some(1.0))
        );
    }
}

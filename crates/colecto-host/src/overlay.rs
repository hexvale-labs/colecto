//! Strict declarative sprite output from an isolated guest.
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub overlay: Overlay,
    pub state: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    pub items: Vec<Item>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub image: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub frame: Frame,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
pub fn validate(bytes: &[u8], assets: &crate::package::Assets) -> Result<Output, String> {
    let output: Output = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if output.state.len() > 4096 || output.overlay.items.len() > 16 {
        return Err("Overlay exceeds limits".into());
    }
    for i in &output.overlay.items {
        if !assets.contains_key(&i.image)
            || ![i.x, i.y, i.frame.x, i.frame.y]
                .iter()
                .all(|n| n.is_finite())
            || i.frame.x < 0.0
            || i.frame.y < 0.0
            || ![i.width, i.height, i.frame.width, i.frame.height]
                .iter()
                .all(|n| n.is_finite() && *n > 0.0 && *n <= 512.0)
        {
            return Err("Invalid overlay item".into());
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_assets_fields_and_excessive_output() {
        let assets = [("cat.png".into(), vec![])].into();
        let item = serde_json::json!({"image":"cat.png","x":0,"y":1,"width":32,"height":32,"frame":{"x":0,"y":0,"width":32,"height":32}});
        let good = serde_json::json!({"overlay":{"items":[item.clone()]},"state":""});
        let check =
            |v: &serde_json::Value| validate(&serde_json::to_vec(v).unwrap(), &assets).is_ok();
        assert!(check(&good));
        for (key, value) in [
            ("image", serde_json::json!("other.png")),
            ("width", serde_json::json!(513)),
            ("height", serde_json::json!(-1)),
            ("script", serde_json::json!("evil")),
        ] {
            let mut bad = good.clone();
            bad["overlay"]["items"][0][key] = value;
            assert!(!check(&bad));
        }
        let mut bad = good.clone();
        bad["state"] = "x".repeat(4097).into();
        assert!(!check(&bad));
        bad = good;
        bad["overlay"]["items"] = serde_json::json!(vec![item; 17]);
        assert!(!check(&bad));
    }
}

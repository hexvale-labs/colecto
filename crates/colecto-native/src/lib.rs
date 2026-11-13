//! Native touches for the Colecto mobile app (Android, iOS).
//!
//! - **Back**: on Android the plugin claims the system back gesture while the page has its own
//!   history to go back through, streams predictive-back progress to the page as
//!   `colecto:back` window events, and goes back in the WebView when the gesture commits.
//!   With nothing left to go back to, the press falls through to Tauri, which leaves the app.
//! - **Press feedback**: the `press` command plays the system click sound (Android) and a light
//!   haptic for toggles (Android and iOS). Android only plays them when the owner has touch
//!   sounds and touch feedback turned on.
//!
//! On desktop the plugin does nothing.
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod ios {
    tauri::ios_plugin_binding!(init_plugin_colecto_native);

    pub(super) fn register<R: tauri::Runtime>(
        api: &tauri::plugin::PluginApi<R, ()>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        api.register_ios_plugin(init_plugin_colecto_native)?;
        Ok(())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("colecto-native")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _api.register_android_plugin("app.colecto.mobile", "NativePlugin")?;
            #[cfg(target_os = "ios")]
            ios::register(_api)?;
            Ok(())
        })
        .build()
}

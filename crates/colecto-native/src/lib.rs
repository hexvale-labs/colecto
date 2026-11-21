//! Native touches for the Colecto mobile app (Android, iOS).
//!
//! - **Back**: on Android the plugin claims the system back gesture while the page has its own
//!   history to go back through, streams predictive-back progress to the page as
//!   `colecto:back` window events, and goes back in the WebView when the gesture commits.
//!   With nothing left to go back to, the press falls through to Tauri, which leaves the app.
//! - **Press feedback**: the `press` command plays the system click sound (Android) and a light
//!   haptic for toggles (Android and iOS). Android only plays them when the owner has touch
//!   sounds and touch feedback turned on.
//! - **Credential storage**: on Android the plugin exposes `Keystore` to Rust code only (never to
//!   the WebView) to wrap vault keys with Android Keystore.
//!
//! On desktop the plugin does nothing.
use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
mod android {
    use serde::{Deserialize, Serialize};
    use std::fmt;
    use tauri::{
        Runtime,
        plugin::{PluginHandle, PluginInvokeError},
    };

    /// Error returned by Android Keystore credential operations.
    #[derive(Debug)]
    pub enum KeystoreError {
        Missing,
        Unavailable(String),
    }

    impl fmt::Display for KeystoreError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Missing => f.write_str("Android Keystore entry not found"),
                Self::Unavailable(message) => f.write_str(message),
            }
        }
    }

    impl std::error::Error for KeystoreError {}

    /// Rust-only bridge to Android Keystore in `NativePlugin`.
    #[derive(Clone)]
    pub struct Keystore<R: Runtime>(pub(super) PluginHandle<R>);

    #[derive(Serialize)]
    struct IdArgs<'a> {
        id: &'a str,
    }

    #[derive(Serialize)]
    struct SetArgs<'a> {
        id: &'a str,
        value: &'a str,
    }

    #[derive(Deserialize)]
    struct ValueResponse {
        value: String,
    }

    fn map_error(error: PluginInvokeError) -> KeystoreError {
        match error {
            PluginInvokeError::InvokeRejected(ref response)
                if response.code.as_deref() == Some("missing") =>
            {
                KeystoreError::Missing
            }
            other => KeystoreError::Unavailable(other.to_string()),
        }
    }

    impl<R: Runtime> Keystore<R> {
        pub fn get(&self, id: &str) -> Result<String, KeystoreError> {
            let response: ValueResponse = self
                .0
                .run_mobile_plugin("secretGet", IdArgs { id })
                .map_err(map_error)?;
            Ok(response.value)
        }

        pub fn set(&self, id: &str, value: &str) -> Result<(), KeystoreError> {
            self.0
                .run_mobile_plugin::<()>("secretSet", SetArgs { id, value })
                .map_err(map_error)
        }

        pub fn delete(&self, id: &str) -> Result<(), KeystoreError> {
            self.0
                .run_mobile_plugin::<()>("secretDelete", IdArgs { id })
                .map_err(map_error)
        }
    }
}

#[cfg(target_os = "android")]
pub use android::{Keystore, KeystoreError};

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
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("app.colecto.mobile", "NativePlugin")?;
                _app.manage(Keystore(handle));
            }
            #[cfg(target_os = "ios")]
            ios::register(&_api)?;
            Ok(())
        })
        .build()
}

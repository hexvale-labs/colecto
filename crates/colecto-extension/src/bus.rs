use serde::Serialize;
use serde::de::DeserializeOwned;

/// A request another extension answers on its bus. The type itself is the payload.
///
/// Declare one with [`bus_request!`](crate::bus_request), which builds the versioned
/// name and checks it at compile time.
pub trait BusRequest: Serialize + DeserializeOwned {
    /// The answering extension's ID (its bus).
    const BUS: &'static str;
    /// Versioned request name, such as `v1/get_draft`.
    const NAME: &'static str;
    /// What the answer deserializes to.
    type Response: Serialize + DeserializeOwned;
}

/// An event published on an extension's bus. The type itself is the payload.
///
/// Declare one with [`bus_event!`](crate::bus_event).
pub trait BusEvent: Serialize + DeserializeOwned {
    /// The publishing extension's ID (its bus).
    const BUS: &'static str;
    /// Versioned event name, such as `v1/message_opened`.
    const NAME: &'static str;
}

/// Implements [`BusRequest`] for a payload type.
///
/// ```ignore
/// #[derive(Serialize, Deserialize)]
/// struct WordCount { message_id: String }
/// #[derive(Serialize, Deserialize)]
/// struct Count { words: usize }
///
/// colecto_extension::bus_request!(WordCount => Count, bus = "word-counter", version = 1, name = "word_count");
/// assert_eq!(WordCount::NAME, "v1/word_count");
/// ```
///
/// An invalid bus ID, a version below 1, or a name the Host would reject is a compile
/// error. Use `()` as the response for requests that are answered with `null`.
#[macro_export]
macro_rules! bus_request {
    ($ty:ty => $response:ty, bus = $bus:literal, version = $version:literal, name = $name:literal $(,)?) => {
        impl $crate::BusRequest for $ty {
            const BUS: &'static str = $bus;
            const NAME: &'static str = concat!("v", $version, "/", $name);
            type Response = $response;
        }
        const _: () =
            $crate::__private::check_names($bus, $version, concat!("v", $version, "/", $name));
    };
}

/// Implements [`BusEvent`] for a payload type.
///
/// ```ignore
/// #[derive(Serialize, Deserialize)]
/// struct Counted { words: usize }
///
/// colecto_extension::bus_event!(Counted, bus = "word-counter", version = 1, name = "counted");
/// ```
#[macro_export]
macro_rules! bus_event {
    ($ty:ty, bus = $bus:literal, version = $version:literal, name = $name:literal $(,)?) => {
        impl $crate::BusEvent for $ty {
            const BUS: &'static str = $bus;
            const NAME: &'static str = concat!("v", $version, "/", $name);
        }
        const _: () =
            $crate::__private::check_names($bus, $version, concat!("v", $version, "/", $name));
    };
}

/// Compile-time check of the Host's bus rules (`crates/colecto-host/src/bus.rs` and
/// extension ID validation). Panicking in a constant makes it a build error.
#[doc(hidden)]
pub const fn check_names(bus: &str, version: u32, name: &str) {
    assert!(version >= 1, "bus versions start at 1");
    let bus = bus.as_bytes();
    assert!(!bus.is_empty(), "bus ID must not be empty");
    let mut i = 0;
    while i < bus.len() {
        let b = bus[i];
        assert!(
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-',
            "bus ID may use only lowercase ASCII letters, digits and hyphens"
        );
        i += 1;
    }
    let name = name.as_bytes();
    assert!(
        name.len() <= 64,
        "bus names are at most 64 bytes, including the version"
    );
    // The macros prefix "v<version>/", so an empty name leaves a trailing slash.
    assert!(name[name.len() - 1] != b'/', "bus name must not be empty");
    let mut i = 0;
    while i < name.len() {
        let b = name[i];
        assert!(
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_' || b == b'/',
            "bus name may use only lowercase ASCII letters, digits, '-', '_' and '/'"
        );
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_host_name_rules() {
        check_names("word-counter", 1, "v1/word_count");
        let rejected = |bus: &'static str, version, name: &'static str| {
            std::panic::catch_unwind(|| check_names(bus, version, name)).is_err()
        };
        assert!(rejected("word-counter", 0, "v0/x"));
        assert!(rejected("Word", 1, "v1/x"));
        assert!(rejected("", 1, "v1/x"));
        assert!(rejected("a", 1, "v1/"));
        assert!(rejected("a", 1, "v1/Upper"));
        assert!(rejected("a", 1, &*"v1/".to_string().repeat(17).leak()));
    }
}

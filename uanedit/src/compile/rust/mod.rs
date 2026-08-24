mod data_types;
mod name;
mod node_ids;

use crate::AddressSpace;

/// The `<base>.rs` file: a safe entry point over the C initialiser, the NodeIds as constants,
/// and the DataTypes as plain Rust.
pub(super) fn wrapper(
    space: &AddressSpace,
    base: &str,
) -> String {
    let count = space.namespaces().len();
    let uris: String = (0..count)
        .map(|index| {
            let uri = u16::try_from(index)
                .ok()
                .and_then(|index| space.namespaces().uri(index))
                .unwrap_or_default();
            format!("    {},\n", name::string_literal(uri))
        })
        .collect();
    let mut out = format!(
        r#"/* WARNING: This is a generated file.
 * Any manual changes will be overwritten. */

//! Safe wrapper around the `{base}` initialiser in `{base}.c`, with the nodeset's NodeIds as
//! constants and its DataTypes as plain Rust.
//!
//! Compile `{base}.c` into the same binary and keep `open62541-sys` in the
//! dependencies; it builds and links open62541 itself, but exports no include
//! path, so the build script points `cc` at headers of the same open62541
//! version the crate bundles:
//!
//! ```text
//! cc::Build::new()
//!     .file("src/{base}.c")
//!     .include("<open62541 include directory>")
//!     .warnings(false)
//!     .compile("{base}");
//! ```

use open62541_sys::{{
    UA_STATUSCODE_GOOD,
    UA_Server,
    UA_StatusCode,
}};

/// The namespace URIs [`insert`] registers, in the order the initialiser asks the server.
pub const NAMESPACE_URIS: [&str; {count}] = [
{uris}];

mod ffi {{
    unsafe extern "C" {{
        #[link_name = "{base}"]
        pub unsafe fn init(server: *mut open62541_sys::UA_Server) -> open62541_sys::UA_StatusCode;
        #[link_name = "{base}_ns"]
        pub static mut NAMESPACE_INDEXES: [u16; {count}];
    }}
}}

/// Inserts the compiled nodeset into `server`.
///
/// # Errors
///
/// The first bad status code a node or reference registration returned.
pub fn insert(server: &mut UA_Server) -> Result<(), UA_StatusCode> {{
    match unsafe {{ ffi::init(server) }} {{
        UA_STATUSCODE_GOOD => Ok(()),
        status => Err(status),
    }}
}}

/// The server's index for each entry of [`NAMESPACE_URIS`] — all zero until [`insert`] filled
/// them, and not to be read while it runs.
pub fn namespace_indexes() -> [u16; {count}] {{
    unsafe {{ ffi::NAMESPACE_INDEXES }}
}}
"#
    );
    out.push_str(&node_ids::section(space));
    out.push_str(&data_types::section(space));
    out
}

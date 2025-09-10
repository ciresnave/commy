use std::ffi::CStr;
use std::path::Path;

use crate::errors::CommyError;
use libloading::Library;

use crate::types::{Formats, Registry, TypeEntry};

#[repr(C)]
pub struct PluginTypeDescriptor {
    pub type_name: *const std::os::raw::c_char,
    pub schema_hash: u64,
    pub formats: u32,
    pub serialize_into: Option<
        extern "C" fn(
            ctx: *mut std::ffi::c_void,
            typed_ptr: *const std::ffi::c_void,
            out_buf: *mut u8,
            out_len: usize,
        ) -> usize,
    >,
    pub get_schema_text:
        Option<extern "C" fn(ctx: *mut std::ffi::c_void) -> *const std::os::raw::c_char>,
    pub ctx: *mut std::ffi::c_void,
}

type GetDescFn = unsafe extern "C" fn(*mut usize) -> *const *const PluginTypeDescriptor;
type RegisterFn = unsafe extern "C" fn(descriptors: *const *const PluginTypeDescriptor, n: usize);

/// A handle that keeps the dynamic library open while the host holds it.
pub struct PluginHandle {
    _lib: Library,
}

impl PluginHandle {
    /// Load a plugin library from `path` and register any descriptors it exposes into `registry`.
    pub fn load<P: AsRef<Path>>(path: P, registry: &Registry) -> Result<Self, CommyError> {
        let lib = unsafe {
            Library::new(path.as_ref()).map_err(|e| CommyError::PluginLoad(format!("{}", e)))?
        };

        unsafe {
            if let Ok(sym) = lib.get::<GetDescFn>(b"com_my_plugin_get_descriptors\0") {
                let get_desc: GetDescFn = *sym;
                let mut n: usize = 0;
                let arr = get_desc(&mut n as *mut usize);
                if !arr.is_null() && n > 0 {
                    let slice = std::slice::from_raw_parts(arr, n);
                    for &ptr in slice.iter() {
                        if ptr.is_null() {
                            continue;
                        }
                        let desc = &*ptr;
                        let cname = CStr::from_ptr(desc.type_name)
                            .to_string_lossy()
                            .into_owned();
                        let formats = Formats::from_bits_truncate(desc.formats);

                        let entry = TypeEntry {
                            type_name: cname,
                            schema_hash: desc.schema_hash,
                            formats,
                            writer: None,
                        };

                        registry.register_plugin(entry);
                    }
                }
            } else if let Ok(sym) = lib.get::<RegisterFn>(b"com_my_plugin_register\0") {
                let reg_fn: RegisterFn = *sym;
                reg_fn(std::ptr::null(), 0);
            }
        }

        Ok(PluginHandle { _lib: lib })
    }
}

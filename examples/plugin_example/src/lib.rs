#![allow(non_camel_case_types)]

use once_cell::sync::Lazy;
use std::ffi::{c_void, CString};
use std::os::raw::c_char;

use commy_macro::rkyv_writer;
use rkyv::{Archive, Serialize};

#[repr(C)]
pub struct PluginTypeDescriptor {
    pub type_name: *const c_char,
    pub schema_hash: u64,
    pub formats: u32,
    pub serialize_into: Option<
        extern "C" fn(
            ctx: *mut c_void,
            typed_ptr: *const c_void,
            out_buf: *mut u8,
            out_len: usize,
        ) -> usize,
    >,
    pub get_schema_text: Option<extern "C" fn(ctx: *mut c_void) -> *const c_char>,
    pub ctx: *mut c_void,
}

// Define a concrete data type that the plugin will provide writers for
#[derive(Archive, Serialize, Debug)]
#[rkyv_writer]
pub struct PluginExample {
    pub value: String,
}

// The proc-macro generates a `write_into_buffer(&self, buf: &mut [u8]) -> Result<usize, ::commy::errors::CommyError>`
// We'll wrap that in an extern "C" function to fit the plugin ABI expected by the loader.
extern "C" fn serialize_into_rkyv(
    _ctx: *mut c_void,
    typed_ptr: *const c_void,
    out_buf: *mut u8,
    out_len: usize,
) -> usize {
    if typed_ptr.is_null() || out_buf.is_null() {
        return 0;
    }

    // Safety: host promises typed_ptr points to a PluginExample
    let example: &PluginExample = unsafe { &*(typed_ptr as *const PluginExample) };

    // Safety: create a mutable slice pointing to the caller-provided buffer
    let out_slice = unsafe { std::slice::from_raw_parts_mut(out_buf, out_len) };
    match example.write_into_buffer(out_slice) {
        Ok(len) => len,
        Err(_) => 0,
    }
}

extern "C" fn get_schema_text_example(_ctx: *mut c_void) -> *const c_char {
    let s = CString::new("type PluginExample struct { value @0 :Text; }").unwrap();
    // Intentional leak for demo lifetime — plugins often return static strings
    Box::into_raw(Box::new(s)) as *const c_char
}

// Provide descriptors via C ABI
#[no_mangle]
pub extern "C" fn com_my_plugin_get_descriptors(
    n: *mut usize,
) -> *const *const PluginTypeDescriptor {
    // Static C string for the type name
    static TYPE_NAME_CSTR: Lazy<CString> =
        Lazy::new(|| CString::new("plugin_example::PluginExample").unwrap());

    // Create a static descriptor and array lazily without using `static mut`.
    static DESCRIPTOR: Lazy<PluginTypeDescriptor> = Lazy::new(|| PluginTypeDescriptor {
        type_name: TYPE_NAME_CSTR.as_ptr(),
        schema_hash: 0xDEADBEEF,
        // Advertise only the rkyv/zerocopy format for this example plugin
        formats: 0x8,
        serialize_into: Some(serialize_into_rkyv),
        get_schema_text: Some(get_schema_text_example),
        ctx: std::ptr::null_mut(),
    });

    static ARRAY: Lazy<[*const PluginTypeDescriptor; 1]> =
        Lazy::new(|| [&*DESCRIPTOR as *const PluginTypeDescriptor]);

    if !n.is_null() {
        unsafe { *n = 1usize };
    }

    ARRAY.as_ptr()
}

// Backwards-compatible register symbol (no-op for this example)
#[no_mangle]
pub extern "C" fn com_my_plugin_register(
    _descriptors: *const *const PluginTypeDescriptor,
    _n: usize,
) {
    // Intentionally empty; hosts can call get_descriptors instead
}

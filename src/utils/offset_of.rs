//! Simple offset calculation utilities for shared memory operations

/// Calculate the byte offset of a field in a struct
/// This is a simplified version for our shared memory demo
#[allow(unused_macros)]
macro_rules! offset_of {
    ($type:ty, $field:ident) => {{
        let dummy = std::mem::MaybeUninit::<$type>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let field_ptr = unsafe { std::ptr::addr_of!((*dummy_ptr).$field) };
        (field_ptr as *const u8).offset_from(dummy_ptr as *const u8) as usize
    }};
}

#[allow(unused_imports)]
pub(crate) use offset_of;

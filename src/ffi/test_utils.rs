#[cfg(test)]
use crate::ffi::{commy_start_mesh, commy_stop_mesh, CommyError, CommyHandle};

#[cfg(test)]
pub fn assert_invalid_handle_start_stop_unit() {
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };

    assert_eq!(
        unsafe { commy_start_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
    assert_eq!(
        unsafe { commy_stop_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
}

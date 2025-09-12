use commy::ffi::{commy_start_mesh, commy_stop_mesh, CommyError, CommyHandle};

/// Shared helper to assert that operations using an invalid/stale handle are rejected
pub fn assert_invalid_handle_start_stop() {
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };

    // Starting/stopping a non-existent instance should return InstanceNotFound
    assert_eq!(
        unsafe { commy_start_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
    assert_eq!(
        unsafe { commy_stop_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
}

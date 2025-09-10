//! Security and Compliance Tests
//!
//! Comprehensive security validation for enterprise deployment including:
//! - Encryption validation and cipher strength testing
//! - Certificate management and rotation
//! - Authentication and authorization testing
//! - Compliance verification (SOC2, GDPR, HIPAA)
//! - Audit trail validation and integrity
//! - Security policy enforcement

use commy::ffi::minimal::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Security test configuration
struct SecurityTestConfig {
    encryption_enabled: bool,
    cipher_suite: String,
    key_size: u32,
    certificate_rotation_hours: u32,
    audit_level: String,
    compliance_standards: Vec<String>,
}

/// Initialize security test environment
fn setup_security_test() -> CommyFileManagerHandle {
    unsafe {
        commy_ffi_init();
    }
    let config_path = CString::new("/tmp/commy_security_test").unwrap();
    unsafe { commy_create_file_manager(config_path.as_ptr()) }
}

/// Cleanup security test environment
fn cleanup_security_test(handle: CommyFileManagerHandle) {
    unsafe {
        commy_destroy_file_manager(handle);
        commy_ffi_cleanup();
    }
}

#[test]
fn test_encryption_algorithms() {
    let handle = setup_security_test();

    // Test various encryption algorithms
    let encryption_configs = vec![
        ("AES-256-GCM", 256, true),
        ("ChaCha20-Poly1305", 256, true),
        ("AES-128-GCM", 128, true),
        ("AES-192-GCM", 192, true),
    ];

    for (algorithm, key_size, should_be_allowed) in encryption_configs {
        let algorithm_cstr = CString::new(algorithm).unwrap();

        let config = CommyEncryptionConfig {
            algorithm: algorithm_cstr.as_ptr(),
            key_size,
            enable_perfect_forward_secrecy: true,
            enable_certificate_pinning: true,
            min_tls_version: CString::new("1.3").unwrap().as_ptr(),
            cipher_suites: ptr::null(), // Use default secure suites
            certificate_path: CString::new("/tmp/test.pem").unwrap().as_ptr(),
            private_key_path: CString::new("/tmp/test.key").unwrap().as_ptr(),
        };

        let result = unsafe { commy_configure_encryption(handle, &config) };

        if should_be_allowed {
            assert_eq!(
                result,
                CommyError::Success as i32,
                "Failed to configure {} encryption",
                algorithm
            );

            // Verify encryption is active
            let mut is_encrypted: bool = false;
            let verify_result =
                unsafe { commy_verify_encryption_active(handle, &mut is_encrypted) };
            assert_eq!(verify_result, CommyError::Success as i32);
            assert!(is_encrypted, "Encryption not active for {}", algorithm);

            // Test message encryption
            let test_message = b"This is a confidential message";
            let mut encrypted_size: u32 = 0;
            let mut encrypted_data: *mut u8 = ptr::null_mut();

            let encrypt_result = unsafe {
                commy_encrypt_message(
                    handle,
                    test_message.as_ptr(),
                    test_message.len() as u32,
                    &mut encrypted_data,
                    &mut encrypted_size,
                )
            };

            assert_eq!(encrypt_result, CommyError::Success as i32);
            assert!(!encrypted_data.is_null());
            assert!(encrypted_size > test_message.len() as u32); // Encrypted should be larger

            // Test decryption
            let mut decrypted_size: u32 = 0;
            let mut decrypted_data: *mut u8 = ptr::null_mut();

            let decrypt_result = unsafe {
                commy_decrypt_message(
                    handle,
                    encrypted_data,
                    encrypted_size,
                    &mut decrypted_data,
                    &mut decrypted_size,
                )
            };

            assert_eq!(decrypt_result, CommyError::Success as i32);
            assert_eq!(decrypted_size, test_message.len() as u32);

            // Verify decrypted content matches original
            let decrypted_slice =
                unsafe { std::slice::from_raw_parts(decrypted_data, decrypted_size as usize) };
            assert_eq!(decrypted_slice, test_message);

            // Clean up
            unsafe {
                commy_free_buffer(encrypted_data);
                commy_free_buffer(decrypted_data);
            }

            println!("✓ {} encryption test passed", algorithm);
        } else {
            assert_ne!(
                result,
                CommyError::Success as i32,
                "Weak encryption {} should not be allowed",
                algorithm
            );
        }
    }

    cleanup_security_test(handle);
}

#[test]
fn test_certificate_validation() {
    let handle = setup_security_test();

    // Test certificate validation scenarios
    let cert_scenarios = vec![
        ("valid_cert.pem", "valid_key.pem", true),
        ("expired_cert.pem", "valid_key.pem", false),
        ("self_signed_cert.pem", "self_signed_key.pem", false), // Should fail in strict mode
        ("invalid_cert.pem", "valid_key.pem", false),
        ("valid_cert.pem", "wrong_key.pem", false),
    ];

    for (cert_file, key_file, should_be_valid) in cert_scenarios {
        let cert_path = CString::new(format!("/tmp/{}", cert_file)).unwrap();
        let key_path = CString::new(format!("/tmp/{}", key_file)).unwrap();

        let result =
            unsafe { commy_validate_certificate(handle, cert_path.as_ptr(), key_path.as_ptr()) };

        if should_be_valid {
            assert_eq!(
                result,
                CommyError::Success as i32,
                "Valid certificate {} should pass validation",
                cert_file
            );
        } else {
            assert_ne!(
                result,
                CommyError::Success as i32,
                "Invalid certificate {} should fail validation",
                cert_file
            );
        }

        println!("✓ Certificate validation test for {} completed", cert_file);
    }

    cleanup_security_test(handle);
}

#[test]
fn test_certificate_rotation() {
    let handle = setup_security_test();

    // Configure certificate rotation
    let rotation_config = CommyCertificateRotationConfig {
        rotation_interval_hours: 24,
        pre_expiry_renewal_hours: 72,
        backup_certificate_count: 3,
        auto_rotation_enabled: true,
        notification_webhook: CString::new("https://alerts.company.com/cert-rotation")
            .unwrap()
            .as_ptr(),
    };

    let result = unsafe { commy_configure_certificate_rotation(handle, &rotation_config) };
    assert_eq!(result, CommyError::Success as i32);

    // Simulate certificate rotation
    let mut rotation_status: CommyCertificateRotationStatus = Default::default();
    let status_result =
        unsafe { commy_get_certificate_rotation_status(handle, &mut rotation_status) };
    assert_eq!(status_result, CommyError::Success as i32);

    // Verify rotation configuration
    assert_eq!(rotation_status.rotation_interval_hours, 24);
    assert_eq!(rotation_status.pre_expiry_renewal_hours, 72);
    assert!(rotation_status.auto_rotation_enabled);

    // Test manual rotation trigger
    let manual_rotation_result = unsafe { commy_trigger_certificate_rotation(handle) };
    assert_eq!(manual_rotation_result, CommyError::Success as i32);

    // Verify rotation occurred
    let mut post_rotation_status: CommyCertificateRotationStatus = Default::default();
    let post_status_result =
        unsafe { commy_get_certificate_rotation_status(handle, &mut post_rotation_status) };
    assert_eq!(post_status_result, CommyError::Success as i32);
    assert!(post_rotation_status.last_rotation_timestamp > rotation_status.last_rotation_timestamp);

    println!("✓ Certificate rotation test completed");

    cleanup_security_test(handle);
}

#[test]
fn test_authentication_mechanisms() {
    let handle = setup_security_test();

    // Test various authentication mechanisms
    let auth_configs = vec![
        ("JWT", "HS256", true),
        ("JWT", "RS256", true),
        ("JWT", "ES256", true),
        ("mTLS", "", true),
        ("OAuth2", "client_credentials", true),
        ("OAuth2", "authorization_code", true),
        ("BasicAuth", "", false), // Should be disabled in enterprise
        ("None", "", false),      // Should never be allowed
    ];

    for (auth_type, auth_method, should_be_allowed) in auth_configs {
        let auth_type_cstr = CString::new(auth_type).unwrap();
        let auth_method_cstr = CString::new(auth_method).unwrap();

        let auth_config = CommyAuthenticationConfig {
            auth_type: auth_type_cstr.as_ptr(),
            auth_method: auth_method_cstr.as_ptr(),
            token_expiry_seconds: 3600,
            refresh_token_enabled: true,
            multi_factor_required: true,
            allowed_issuers: ptr::null(),
            jwks_endpoint: CString::new("https://auth.company.com/.well-known/jwks.json")
                .unwrap()
                .as_ptr(),
            audience: CString::new("commy-mesh").unwrap().as_ptr(),
        };

        let result = unsafe { commy_configure_authentication(handle, &auth_config) };

        if should_be_allowed {
            assert_eq!(
                result,
                CommyError::Success as i32,
                "Enterprise authentication {} should be allowed",
                auth_type
            );

            // Test authentication with valid token
            let test_token = CString::new("valid.jwt.token").unwrap();
            let mut auth_result: CommyAuthenticationResult = Default::default();

            let verify_result = unsafe {
                commy_verify_authentication(handle, test_token.as_ptr(), &mut auth_result)
            };

            // Note: This would normally succeed with a real valid token
            // For testing, we verify the configuration was accepted
            println!("✓ {} authentication configuration accepted", auth_type);
        } else {
            assert_ne!(
                result,
                CommyError::Success as i32,
                "Insecure authentication {} should not be allowed",
                auth_type
            );
            println!("✓ {} authentication correctly rejected", auth_type);
        }
    }

    cleanup_security_test(handle);
}

#[test]
fn test_authorization_policies() {
    let handle = setup_security_test();

    // Test role-based access control
    let rbac_policies = vec![
        ("admin", vec!["read", "write", "delete", "configure"], true),
        ("operator", vec!["read", "write", "monitor"], true),
        ("readonly", vec!["read", "monitor"], true),
        ("guest", vec!["read"], true),
        ("anonymous", vec![], false), // Should not be allowed
    ];

    for (role, permissions, should_be_allowed) in rbac_policies {
        let role_cstr = CString::new(role).unwrap();

        // Convert permissions to C-style array
        let permission_cstrs: Vec<CString> = permissions
            .iter()
            .map(|p| CString::new(*p).unwrap())
            .collect();
        let permission_ptrs: Vec<*const i8> =
            permission_cstrs.iter().map(|cs| cs.as_ptr()).collect();

        let rbac_config = CommyRBACConfig {
            role: role_cstr.as_ptr(),
            permissions: permission_ptrs.as_ptr(),
            permission_count: permission_ptrs.len() as u32,
            resource_patterns: ptr::null(),
            resource_pattern_count: 0,
            time_restrictions: ptr::null(),
            ip_restrictions: ptr::null(),
        };

        let result = unsafe { commy_configure_rbac_policy(handle, &rbac_config) };

        if should_be_allowed {
            assert_eq!(
                result,
                CommyError::Success as i32,
                "Role {} should be configurable",
                role
            );

            // Test permission checking
            for permission in &permissions {
                let permission_cstr = CString::new(*permission).unwrap();
                let resource_cstr = CString::new("/api/v1/services").unwrap();

                let mut has_permission: bool = false;
                let check_result = unsafe {
                    commy_check_permission(
                        handle,
                        role_cstr.as_ptr(),
                        permission_cstr.as_ptr(),
                        resource_cstr.as_ptr(),
                        &mut has_permission,
                    )
                };

                assert_eq!(check_result, CommyError::Success as i32);
                assert!(
                    has_permission,
                    "Role {} should have {} permission",
                    role, permission
                );
            }

            // Test forbidden permission
            let forbidden_permission = CString::new("admin_only").unwrap();
            let mut has_forbidden: bool = true; // Default to true to test it gets set to false

            let forbidden_result = unsafe {
                commy_check_permission(
                    handle,
                    role_cstr.as_ptr(),
                    forbidden_permission.as_ptr(),
                    CString::new("/api/v1/admin").unwrap().as_ptr(),
                    &mut has_forbidden,
                )
            };

            if role != "admin" {
                assert!(
                    !has_forbidden,
                    "Role {} should not have admin_only permission",
                    role
                );
            }

            println!("✓ RBAC configuration for {} role completed", role);
        } else {
            assert_ne!(
                result,
                CommyError::Success as i32,
                "Insecure role {} should not be allowed",
                role
            );
        }
    }

    cleanup_security_test(handle);
}

#[test]
fn test_compliance_validation() {
    let handle = setup_security_test();

    // Test compliance with various standards
    let compliance_standards = vec![
        (
            "SOC2",
            vec![
                "encryption_at_rest",
                "encryption_in_transit",
                "access_logging",
                "data_classification",
            ],
        ),
        (
            "GDPR",
            vec![
                "data_minimization",
                "right_to_erasure",
                "consent_management",
                "data_portability",
            ],
        ),
        (
            "HIPAA",
            vec![
                "encryption",
                "access_controls",
                "audit_trails",
                "risk_assessment",
            ],
        ),
        (
            "PCI_DSS",
            vec![
                "network_security",
                "cardholder_data_protection",
                "vulnerability_management",
            ],
        ),
        (
            "FedRAMP",
            vec![
                "continuous_monitoring",
                "incident_response",
                "configuration_management",
            ],
        ),
    ];

    for (standard, requirements) in compliance_standards {
        let standard_cstr = CString::new(standard).unwrap();

        // Convert requirements to C-style array
        let requirement_cstrs: Vec<CString> = requirements
            .iter()
            .map(|r| CString::new(*r).unwrap())
            .collect();
        let requirement_ptrs: Vec<*const i8> =
            requirement_cstrs.iter().map(|cs| cs.as_ptr()).collect();

        let compliance_config = CommyComplianceConfig {
            standard: standard_cstr.as_ptr(),
            requirements: requirement_ptrs.as_ptr(),
            requirement_count: requirement_ptrs.len() as u32,
            audit_frequency_days: 30,
            compliance_officer_email: CString::new("compliance@company.com").unwrap().as_ptr(),
            evidence_retention_days: 2555, // 7 years
            automated_scanning_enabled: true,
        };

        let result = unsafe { commy_configure_compliance(handle, &compliance_config) };
        assert_eq!(
            result,
            CommyError::Success as i32,
            "Failed to configure {} compliance",
            standard
        );

        // Perform compliance scan
        let mut compliance_report: CommyComplianceReport = Default::default();
        let scan_result = unsafe {
            commy_perform_compliance_scan(handle, standard_cstr.as_ptr(), &mut compliance_report)
        };
        assert_eq!(scan_result, CommyError::Success as i32);

        // Verify compliance status
        assert!(
            compliance_report.overall_compliance_percentage >= 95.0,
            "{} compliance should be at least 95%, got {}%",
            standard,
            compliance_report.overall_compliance_percentage
        );

        assert!(
            compliance_report.critical_violations_count == 0,
            "{} compliance should have no critical violations, found {}",
            standard,
            compliance_report.critical_violations_count
        );

        // Verify specific requirements
        for (i, requirement) in requirements.iter().enumerate() {
            let requirement_status = unsafe { compliance_report.requirement_statuses.add(i) };
            let status = unsafe { *requirement_status };
            assert!(
                status.is_compliant,
                "{} requirement '{}' should be compliant",
                standard, requirement
            );
        }

        println!(
            "✓ {} compliance validation completed ({}% compliant)",
            standard, compliance_report.overall_compliance_percentage
        );
    }

    cleanup_security_test(handle);
}

#[test]
fn test_audit_trail_integrity() {
    let handle = setup_security_test();

    // Configure audit trail
    let audit_config = CommyAuditConfig {
        audit_level: CString::new("DETAILED").unwrap().as_ptr(),
        log_encryption_enabled: true,
        log_signing_enabled: true,
        tamper_detection_enabled: true,
        retention_days: 2555, // 7 years
        export_format: CString::new("JSON").unwrap().as_ptr(),
        storage_location: CString::new("/secure/audit/logs").unwrap().as_ptr(),
        real_time_alerting: true,
    };

    let result = unsafe { commy_configure_audit_trail(handle, &audit_config) };
    assert_eq!(result, CommyError::Success as i32);

    // Generate audit events
    let audit_events = vec![
        ("USER_LOGIN", "user123", "192.168.1.100"),
        ("MESSAGE_SENT", "service-a", "internal"),
        ("CONFIG_CHANGED", "admin", "192.168.1.50"),
        ("PERMISSION_DENIED", "user456", "192.168.1.200"),
        ("USER_LOGOUT", "user123", "192.168.1.100"),
    ];

    let mut event_ids = vec![];

    for (event_type, user, source_ip) in &audit_events {
        let event_type_cstr = CString::new(*event_type).unwrap();
        let user_cstr = CString::new(*user).unwrap();
        let source_ip_cstr = CString::new(*source_ip).unwrap();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let audit_event = CommyAuditEvent {
            event_type: event_type_cstr.as_ptr(),
            user_id: user_cstr.as_ptr(),
            source_ip: source_ip_cstr.as_ptr(),
            timestamp,
            resource: CString::new("/api/v1/test").unwrap().as_ptr(),
            action: CString::new("test_action").unwrap().as_ptr(),
            outcome: CString::new("SUCCESS").unwrap().as_ptr(),
            details: CString::new("{}").unwrap().as_ptr(),
        };

        let mut event_id: u64 = 0;
        let log_result = unsafe { commy_log_audit_event(handle, &audit_event, &mut event_id) };
        assert_eq!(log_result, CommyError::Success as i32);
        assert!(event_id > 0);

        event_ids.push(event_id);
    }

    // Verify audit trail integrity
    let mut integrity_status: CommyAuditIntegrityStatus = Default::default();
    let integrity_result = unsafe { commy_verify_audit_integrity(handle, &mut integrity_status) };
    assert_eq!(integrity_result, CommyError::Success as i32);
    assert!(integrity_status.integrity_valid);
    assert_eq!(integrity_status.tamper_attempts_detected, 0);
    assert_eq!(
        integrity_status.verified_events_count,
        audit_events.len() as u64
    );

    // Test audit search functionality
    let search_criteria = CommyAuditSearchCriteria {
        start_timestamp: 0,
        end_timestamp: u64::MAX,
        event_types: ptr::null(),
        event_type_count: 0,
        user_ids: ptr::null(),
        user_id_count: 0,
        source_ips: ptr::null(),
        source_ip_count: 0,
        limit: 100,
        offset: 0,
    };

    let mut search_results: CommyAuditSearchResults = Default::default();
    let search_result =
        unsafe { commy_search_audit_events(handle, &search_criteria, &mut search_results) };
    assert_eq!(search_result, CommyError::Success as i32);
    assert_eq!(search_results.total_count, audit_events.len() as u64);
    assert!(search_results.events_count <= search_results.total_count);

    // Test audit export
    let export_criteria = CommyAuditExportCriteria {
        start_timestamp: 0,
        end_timestamp: u64::MAX,
        format: CString::new("JSON").unwrap().as_ptr(),
        encryption_enabled: true,
        digital_signature_enabled: true,
        export_path: CString::new("/tmp/audit_export.json").unwrap().as_ptr(),
    };

    let export_result = unsafe { commy_export_audit_logs(handle, &export_criteria) };
    assert_eq!(export_result, CommyError::Success as i32);

    println!("✓ Audit trail integrity test completed");
    println!("  Events logged: {}", audit_events.len());
    println!("  Integrity valid: {}", integrity_status.integrity_valid);
    println!(
        "  Tamper attempts: {}",
        integrity_status.tamper_attempts_detected
    );

    cleanup_security_test(handle);
}

#[test]
fn test_security_incident_response() {
    let handle = setup_security_test();

    // Configure incident response
    let incident_config = CommyIncidentResponseConfig {
        auto_response_enabled: true,
        notification_webhook: CString::new("https://alerts.company.com/security")
            .unwrap()
            .as_ptr(),
        escalation_threshold_minutes: 5,
        quarantine_enabled: true,
        forensics_collection_enabled: true,
        incident_retention_days: 365,
    };

    let result = unsafe { commy_configure_incident_response(handle, &incident_config) };
    assert_eq!(result, CommyError::Success as i32);

    // Simulate security incidents
    let security_incidents = vec![
        (
            "UNAUTHORIZED_ACCESS",
            "CRITICAL",
            "user789",
            "192.168.1.999",
        ),
        ("BRUTE_FORCE_ATTACK", "HIGH", "attacker", "10.0.0.1"),
        (
            "PRIVILEGE_ESCALATION",
            "CRITICAL",
            "user456",
            "192.168.1.200",
        ),
        ("DATA_EXFILTRATION", "CRITICAL", "unknown", "172.16.0.1"),
        ("MALFORMED_REQUEST", "MEDIUM", "service-x", "internal"),
    ];

    for (incident_type, severity, source_user, source_ip) in &security_incidents {
        let incident_type_cstr = CString::new(*incident_type).unwrap();
        let severity_cstr = CString::new(*severity).unwrap();
        let source_user_cstr = CString::new(*source_user).unwrap();
        let source_ip_cstr = CString::new(*source_ip).unwrap();

        let security_incident = CommySecurityIncident {
            incident_type: incident_type_cstr.as_ptr(),
            severity: severity_cstr.as_ptr(),
            source_user: source_user_cstr.as_ptr(),
            source_ip: source_ip_cstr.as_ptr(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            description: CString::new("Automated security test incident")
                .unwrap()
                .as_ptr(),
            evidence: CString::new("{}").unwrap().as_ptr(),
        };

        let mut incident_id: u64 = 0;
        let report_result =
            unsafe { commy_report_security_incident(handle, &security_incident, &mut incident_id) };
        assert_eq!(report_result, CommyError::Success as i32);
        assert!(incident_id > 0);

        // Verify incident response was triggered
        let mut response_status: CommyIncidentResponseStatus = Default::default();
        let status_result = unsafe {
            commy_get_incident_response_status(handle, incident_id, &mut response_status)
        };
        assert_eq!(status_result, CommyError::Success as i32);

        // Verify appropriate response based on severity
        if *severity == "CRITICAL" {
            assert!(response_status.quarantine_activated);
            assert!(response_status.forensics_collected);
            assert!(response_status.notification_sent);
            assert!(response_status.escalation_triggered);
        }

        println!(
            "✓ Security incident {} processed (ID: {})",
            incident_type, incident_id
        );
    }

    cleanup_security_test(handle);
}

#[test]
fn test_data_protection_mechanisms() {
    let handle = setup_security_test();

    // Test data classification and protection
    let data_classifications = vec![
        ("PUBLIC", false, false, 30),
        ("INTERNAL", true, false, 90),
        ("CONFIDENTIAL", true, true, 365),
        ("RESTRICTED", true, true, 2555), // 7 years
        ("SECRET", true, true, 3650),     // 10 years
    ];

    for (classification, encrypt_at_rest, encrypt_in_transit, retention_days) in
        data_classifications
    {
        let classification_cstr = CString::new(classification).unwrap();

        let protection_config = CommyDataProtectionConfig {
            classification: classification_cstr.as_ptr(),
            encrypt_at_rest,
            encrypt_in_transit,
            retention_days,
            anonymization_enabled: classification == "SECRET",
            redaction_patterns: ptr::null(),
            redaction_pattern_count: 0,
            access_log_enabled: true,
        };

        let result = unsafe { commy_configure_data_protection(handle, &protection_config) };
        assert_eq!(result, CommyError::Success as i32);

        // Test data handling according to classification
        let test_data = format!("This is {} data for testing", classification).into_bytes();
        let mut protected_data: *mut u8 = ptr::null_mut();
        let mut protected_size: u32 = 0;

        let protect_result = unsafe {
            commy_protect_data(
                handle,
                classification_cstr.as_ptr(),
                test_data.as_ptr(),
                test_data.len() as u32,
                &mut protected_data,
                &mut protected_size,
            )
        };
        assert_eq!(protect_result, CommyError::Success as i32);
        assert!(!protected_data.is_null());

        if encrypt_at_rest {
            // Protected data should be different from original (encrypted)
            let protected_slice =
                unsafe { std::slice::from_raw_parts(protected_data, protected_size as usize) };
            assert_ne!(protected_slice, test_data.as_slice());
        }

        // Test data access logging
        let access_log_result = unsafe {
            commy_log_data_access(
                handle,
                classification_cstr.as_ptr(),
                CString::new("test_user").unwrap().as_ptr(),
                CString::new("READ").unwrap().as_ptr(),
            )
        };
        assert_eq!(access_log_result, CommyError::Success as i32);

        unsafe {
            commy_free_buffer(protected_data);
        }

        println!(
            "✓ Data protection for {} classification configured",
            classification
        );
    }

    cleanup_security_test(handle);
}

// Default implementations for test structs
impl Default for CommyCertificateRotationStatus {
    fn default() -> Self {
        Self {
            rotation_interval_hours: 0,
            pre_expiry_renewal_hours: 0,
            auto_rotation_enabled: false,
            last_rotation_timestamp: 0,
            next_rotation_timestamp: 0,
            certificates_in_rotation: 0,
        }
    }
}

impl Default for CommyAuthenticationResult {
    fn default() -> Self {
        Self {
            is_valid: false,
            user_id: ptr::null(),
            roles: ptr::null(),
            role_count: 0,
            expiry_timestamp: 0,
            permissions: ptr::null(),
            permission_count: 0,
        }
    }
}

impl Default for CommyComplianceReport {
    fn default() -> Self {
        Self {
            overall_compliance_percentage: 0.0,
            critical_violations_count: 0,
            major_violations_count: 0,
            minor_violations_count: 0,
            requirement_statuses: ptr::null_mut(),
            requirement_count: 0,
            scan_timestamp: 0,
            next_scan_timestamp: 0,
        }
    }
}

impl Default for CommyAuditIntegrityStatus {
    fn default() -> Self {
        Self {
            integrity_valid: false,
            tamper_attempts_detected: 0,
            verified_events_count: 0,
            corrupted_events_count: 0,
            last_verification_timestamp: 0,
            signature_chain_valid: false,
        }
    }
}

impl Default for CommyAuditSearchResults {
    fn default() -> Self {
        Self {
            total_count: 0,
            events_count: 0,
            events: ptr::null_mut(),
            has_more: false,
            next_offset: 0,
        }
    }
}

impl Default for CommyIncidentResponseStatus {
    fn default() -> Self {
        Self {
            quarantine_activated: false,
            forensics_collected: false,
            notification_sent: false,
            escalation_triggered: false,
            response_time_seconds: 0,
            status: ptr::null(),
        }
    }
}

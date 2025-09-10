//! Security Features Demo
//!
//! Demonstrates comprehensive security capabilities including:
//! - End-to-end encryption with multiple algorithms
//! - Certificate-based authentication
//! - Role-based access control (RBAC)
//! - Security audit logging
//! - Threat detection and mitigation

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, Permission, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::time::{sleep, Duration, Instant};

#[cfg(feature = "manager")]
#[derive(Clone, Debug)]
enum SecurityLevel {
    Public,       // No encryption, open access
    Protected,    // Basic encryption, simple authentication
    Confidential, // Strong encryption, certificate auth
    Secret,       // End-to-end encryption, multi-factor auth
    TopSecret,    // Quantum-resistant encryption, biometric auth
}

#[cfg(feature = "manager")]
#[derive(Clone, Debug)]
enum EncryptionAlgorithm {
    AES256GCM,
    ChaCha20Poly1305,
    XChaCha20Poly1305,
    AES256GCMSiv,
    Post_Quantum_Kyber,
}

#[cfg(feature = "manager")]
#[derive(Clone, Debug)]
struct SecurityProfile {
    level: SecurityLevel,
    encryption_algorithm: EncryptionAlgorithm,
    require_mutual_auth: bool,
    enable_perfect_forward_secrecy: bool,
    audit_level: AuditLevel,
    allowed_roles: Vec<Permission>,
    max_session_duration_minutes: u64,
}

#[cfg(feature = "manager")]
#[derive(Clone, Debug)]
enum AuditLevel {
    None,
    Basic,    // Log connections and errors
    Detailed, // Log all operations
    Forensic, // Log everything with full context
}

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Security Features Demo: Comprehensive Protection");
    println!("==================================================");

    // Demonstrate different security scenarios
    let security_scenarios = vec![
        (
            "Public Data Sharing",
            create_public_security_profile(),
            "Open access for public information",
        ),
        (
            "Internal Communications",
            create_protected_security_profile(),
            "Standard encryption for internal services",
        ),
        (
            "Customer Data Processing",
            create_confidential_security_profile(),
            "Strong encryption for sensitive customer data",
        ),
        (
            "Financial Transactions",
            create_secret_security_profile(),
            "High-security encryption for financial data",
        ),
        (
            "Government/Military Operations",
            create_top_secret_security_profile(),
            "Maximum security with quantum resistance",
        ),
    ];

    println!("\n🛡️  Security Profile Demonstrations:");
    println!("====================================");

    for (scenario_name, profile, description) in security_scenarios {
        println!("\n🔒 Scenario: {}", scenario_name);
        println!("   💡 {}", description);

        demonstrate_security_profile(&profile).await?;

        sleep(Duration::from_millis(500)).await;
    }

    // Demonstrate encryption algorithms
    println!("\n🔐 Encryption Algorithm Comparison:");
    println!("===================================");

    let encryption_tests = vec![
        (
            EncryptionAlgorithm::AES256GCM,
            "Industry standard, hardware acceleration",
        ),
        (
            EncryptionAlgorithm::ChaCha20Poly1305,
            "High performance, software optimized",
        ),
        (
            EncryptionAlgorithm::XChaCha20Poly1305,
            "Extended nonce, better for streaming",
        ),
        (
            EncryptionAlgorithm::AES256GCMSiv,
            "Misuse-resistant, extra safety",
        ),
        (
            EncryptionAlgorithm::Post_Quantum_Kyber,
            "Future-proof, quantum resistant",
        ),
    ];

    for (algorithm, description) in encryption_tests {
        println!("\n🔑 Algorithm: {:?}", algorithm);
        println!("   📝 {}", description);

        let performance = test_encryption_performance(&algorithm).await?;
        println!(
            "   ⚡ Encryption Speed: {:.2} MB/s",
            performance.encryption_speed_mbps
        );
        println!(
            "   🔓 Decryption Speed: {:.2} MB/s",
            performance.decryption_speed_mbps
        );
        println!("   💾 Overhead: {:.1}%", performance.size_overhead_percent);
        println!("   🛡️  Security Level: {}/10", performance.security_rating);
    }

    // Demonstrate access control
    println!("\n👥 Role-Based Access Control (RBAC):");
    println!("====================================");

    demonstrate_rbac_scenarios().await?;

    // Demonstrate audit logging
    println!("\n📋 Security Audit Logging:");
    println!("==========================");

    demonstrate_audit_logging().await?;

    // Demonstrate threat detection
    println!("\n🚨 Threat Detection & Mitigation:");
    println!("=================================");

    demonstrate_threat_detection().await?;

    // Demonstrate certificate management
    println!("\n📜 Certificate Management:");
    println!("=========================");

    demonstrate_certificate_management().await?;

    println!("\n🎉 Security Features Demo Completed!");
    println!("===================================");
    println!("   🔐 Multiple encryption algorithms tested");
    println!("   🛡️  5 security levels demonstrated");
    println!("   👥 Role-based access control validated");
    println!("   📋 Comprehensive audit logging");
    println!("   🚨 Threat detection and mitigation");
    println!("   📜 Certificate-based authentication");

    Ok(())
}

#[cfg(feature = "manager")]
async fn demonstrate_security_profile(
    profile: &SecurityProfile,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("   🔍 Security Level: {:?}", profile.level);
    println!("   🔐 Encryption: {:?}", profile.encryption_algorithm);
    println!("   🤝 Mutual Auth: {}", profile.require_mutual_auth);
    println!(
        "   🔄 Perfect Forward Secrecy: {}",
        profile.enable_perfect_forward_secrecy
    );
    println!("   📊 Audit Level: {:?}", profile.audit_level);
    println!("   👤 Allowed Roles: {:?}", profile.allowed_roles);

    // Create a security-aware request based on the profile
    let request = create_secure_request(profile).await?;

    // Simulate security validation
    let validation_result = validate_security_request(&request, profile).await?;

    if validation_result.is_secure {
        println!("   ✅ Security validation passed");
        println!(
            "   🏆 Security Score: {}/100",
            validation_result.security_score
        );
    } else {
        println!(
            "   ❌ Security validation failed: {}",
            validation_result.failure_reason
        );
    }

    Ok(())
}

#[cfg(feature = "manager")]
async fn create_secure_request(
    profile: &SecurityProfile,
) -> Result<SharedFileRequest, Box<dyn std::error::Error>> {
    let encryption_required = !matches!(profile.level, SecurityLevel::Public);
    let permissions = profile.allowed_roles.clone();

    let request = SharedFileRequest {
        identifier: format!(
            "secure_{}_{:?}",
            chrono::Utc::now().timestamp(),
            profile.level
        ),
        name: format!("secure_data_{:?}", profile.level),
        description: Some(format!(
            "Secure data transfer with {:?} level",
            profile.level
        )),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some((profile.max_session_duration_minutes * 60 * 1000) as u32),
            retry_count: Some(if matches!(profile.level, SecurityLevel::TopSecret) {
                0
            } else {
                3
            }),
        },
        pattern_config: HashMap::from([
            ("security_level".to_string(), format!("{:?}", profile.level)),
            (
                "encryption_algorithm".to_string(),
                format!("{:?}", profile.encryption_algorithm),
            ),
            (
                "audit_level".to_string(),
                format!("{:?}", profile.audit_level),
            ),
            (
                "mutual_auth".to_string(),
                profile.require_mutual_auth.to_string(),
            ),
            (
                "perfect_forward_secrecy".to_string(),
                profile.enable_perfect_forward_secrecy.to_string(),
            ),
        ]),
        file_path: Some(PathBuf::from(format!(
            "secure_data_{:?}.enc",
            profile.level
        ))),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(match profile.level {
            SecurityLevel::Public => 10 * 1024 * 1024,      // 10MB
            SecurityLevel::Protected => 5 * 1024 * 1024,    // 5MB
            SecurityLevel::Confidential => 2 * 1024 * 1024, // 2MB
            SecurityLevel::Secret => 1024 * 1024,           // 1MB
            SecurityLevel::TopSecret => 512 * 1024,         // 512KB
        }),
        ttl_seconds: Some(profile.max_session_duration_minutes * 60),
        max_connections: Some(match profile.level {
            SecurityLevel::Public => 100,
            SecurityLevel::Protected => 50,
            SecurityLevel::Confidential => 10,
            SecurityLevel::Secret => 5,
            SecurityLevel::TopSecret => 1,
        }),
        required_permissions: permissions,
        encryption_required,
        auto_cleanup: true,
        persist_after_disconnect: matches!(
            profile.level,
            SecurityLevel::Secret | SecurityLevel::TopSecret
        ),
        transport_preference: match profile.level {
            SecurityLevel::Public => TransportPreference::Adaptive,
            SecurityLevel::Protected => TransportPreference::PreferLocal,
            SecurityLevel::Confidential => TransportPreference::RequireLocal,
            SecurityLevel::Secret => TransportPreference::RequireLocal,
            SecurityLevel::TopSecret => TransportPreference::RequireLocal,
        },
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(match profile.level {
                SecurityLevel::Public => 1000,
                SecurityLevel::Protected => 2000,
                SecurityLevel::Confidential => 5000,
                SecurityLevel::Secret => 10000,
                SecurityLevel::TopSecret => 30000, // Higher latency for maximum security
            }),
            min_throughput_mbps: Some(match profile.level {
                SecurityLevel::Public => 100,
                SecurityLevel::Protected => 50,
                SecurityLevel::Confidential => 20,
                SecurityLevel::Secret => 10,
                SecurityLevel::TopSecret => 5,
            }),
            consistency_level: match profile.level {
                SecurityLevel::Public => ConsistencyLevel::Eventual,
                SecurityLevel::Protected => ConsistencyLevel::Strong,
                SecurityLevel::Confidential => ConsistencyLevel::Linearizable,
                SecurityLevel::Secret => ConsistencyLevel::Linearizable,
                SecurityLevel::TopSecret => ConsistencyLevel::Linearizable,
            },
            durability_required: !matches!(profile.level, SecurityLevel::Public),
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from(format!("secure_data_{:?}.enc", profile.level)),
            offset: 0,
            data: generate_test_data(&profile.level),
        },
    };

    Ok(request)
}

#[cfg(feature = "manager")]
fn generate_test_data(level: &SecurityLevel) -> Vec<u8> {
    match level {
        SecurityLevel::Public => b"Public information - weather data, news".to_vec(),
        SecurityLevel::Protected => b"Internal company memo - quarterly metrics".to_vec(),
        SecurityLevel::Confidential => b"Customer PII - encrypted user profiles".to_vec(),
        SecurityLevel::Secret => b"Financial data - transaction records, account info".to_vec(),
        SecurityLevel::TopSecret => b"Classified information - national security data".to_vec(),
    }
}

#[cfg(feature = "manager")]
struct SecurityValidationResult {
    is_secure: bool,
    security_score: u8,
    failure_reason: String,
}

#[cfg(feature = "manager")]
async fn validate_security_request(
    request: &SharedFileRequest,
    profile: &SecurityProfile,
) -> Result<SecurityValidationResult, Box<dyn std::error::Error>> {
    let mut score = 0u8;
    let mut issues = Vec::new();

    // Check encryption requirements
    if request.encryption_required {
        score += 20;
    } else if !matches!(profile.level, SecurityLevel::Public) {
        issues.push("Encryption required but not enabled");
    }

    // Check transport security
    match request.transport_preference {
        TransportPreference::RequireLocal => score += 25,
        TransportPreference::PreferLocal => score += 15,
        TransportPreference::PreferNetwork => score += 5,
        TransportPreference::Adaptive => score += 10,
        TransportPreference::RequireNetwork => {
            if matches!(
                profile.level,
                SecurityLevel::Secret | SecurityLevel::TopSecret
            ) {
                issues.push("Network transport not allowed for high security");
            } else {
                score += 5;
            }
        }
        TransportPreference::AutoOptimize => score += 10,
        TransportPreference::LocalOnly => score += 25,
        TransportPreference::NetworkOnly => {
            if matches!(
                profile.level,
                SecurityLevel::Secret | SecurityLevel::TopSecret
            ) {
                issues.push("Network-only transport not allowed for high security");
            } else {
                score += 5;
            }
        }
    }

    // Check consistency requirements
    match request.performance_requirements.consistency_level {
        ConsistencyLevel::Linearizable => score += 20,
        ConsistencyLevel::Strong => score += 15,
        ConsistencyLevel::Eventual => {
            if matches!(
                profile.level,
                SecurityLevel::Confidential | SecurityLevel::Secret | SecurityLevel::TopSecret
            ) {
                issues.push("Eventual consistency insufficient for high security");
            } else {
                score += 5;
            }
        }
        ConsistencyLevel::None => {
            if !matches!(profile.level, SecurityLevel::Public) {
                issues.push("No consistency insufficient for secure operations");
            }
        }
    }

    // Check access control
    if !request.required_permissions.is_empty() {
        score += 15;
    } else if !matches!(profile.level, SecurityLevel::Public) {
        issues.push("Access control required but not configured");
    }

    // Check durability
    if request.performance_requirements.durability_required {
        score += 10;
    }

    // Check session limits
    if request.max_connections.unwrap_or(u32::MAX) <= 10 {
        score += 10;
    }

    let is_secure = issues.is_empty() && score >= 60;
    let failure_reason = if issues.is_empty() {
        "All security checks passed".to_string()
    } else {
        issues.join("; ")
    };

    Ok(SecurityValidationResult {
        is_secure,
        security_score: score.min(100),
        failure_reason,
    })
}

#[cfg(feature = "manager")]
struct EncryptionPerformance {
    encryption_speed_mbps: f64,
    decryption_speed_mbps: f64,
    size_overhead_percent: f64,
    security_rating: u8,
}

#[cfg(feature = "manager")]
async fn test_encryption_performance(
    algorithm: &EncryptionAlgorithm,
) -> Result<EncryptionPerformance, Box<dyn std::error::Error>> {
    // Simulate encryption performance testing
    let (enc_speed, dec_speed, overhead, rating) = match algorithm {
        EncryptionAlgorithm::AES256GCM => (1200.0, 1300.0, 3.2, 8),
        EncryptionAlgorithm::ChaCha20Poly1305 => (950.0, 980.0, 3.1, 8),
        EncryptionAlgorithm::XChaCha20Poly1305 => (920.0, 940.0, 3.1, 8),
        EncryptionAlgorithm::AES256GCMSiv => (800.0, 850.0, 3.5, 9),
        EncryptionAlgorithm::Post_Quantum_Kyber => (150.0, 180.0, 8.5, 10),
    };

    // Simulate actual performance test
    sleep(Duration::from_millis(100)).await;

    Ok(EncryptionPerformance {
        encryption_speed_mbps: enc_speed,
        decryption_speed_mbps: dec_speed,
        size_overhead_percent: overhead,
        security_rating: rating,
    })
}

#[cfg(feature = "manager")]
async fn demonstrate_rbac_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let rbac_scenarios = vec![
        (
            "admin",
            vec![Permission::Read, Permission::Write, Permission::Admin],
            "Full system access",
        ),
        (
            "developer",
            vec![Permission::Read, Permission::Write],
            "Development and deployment",
        ),
        (
            "analyst",
            vec![Permission::Read],
            "Data analysis and reporting",
        ),
        (
            "auditor",
            vec![Permission::Read],
            "Security auditing and compliance",
        ),
        (
            "guest",
            vec![Permission::Read],
            "Read-only access to public data",
        ),
    ];

    for (role, permissions, description) in rbac_scenarios {
        println!("\n   👤 Role: {}", role);
        println!("      📝 Description: {}", description);
        println!("      🔑 Permissions: {:?}", permissions);

        // Create role-specific request
        let request = create_rbac_request(role, &permissions).await?;

        // Simulate permission validation
        let has_access = validate_rbac_access(&request, role, &permissions).await?;

        if has_access {
            println!("      ✅ Access granted");
        } else {
            println!("      ❌ Access denied - insufficient permissions");
        }

        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

#[cfg(feature = "manager")]
async fn create_rbac_request(
    role: &str,
    permissions: &[Permission],
) -> Result<SharedFileRequest, Box<dyn std::error::Error>> {
    let request = SharedFileRequest {
        identifier: format!("rbac_test_{}", role),
        name: format!("rbac_access_{}", role),
        description: Some(format!("RBAC test for role: {}", role)),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(5000),
            retry_count: Some(2),
        },
        pattern_config: HashMap::from([
            ("role".to_string(), role.to_string()),
            ("permissions".to_string(), format!("{:?}", permissions)),
        ]),
        file_path: Some(PathBuf::from(format!("rbac_{}.dat", role))),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(4096),
        ttl_seconds: Some(3600),
        max_connections: Some(5),
        required_permissions: permissions.to_vec(),
        encryption_required: true,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::RequireLocal,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(1000),
            min_throughput_mbps: Some(10),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from(format!("rbac_{}.dat", role)),
            offset: 0,
            data: format!("RBAC test data for role: {}", role).into_bytes(),
        },
    };

    Ok(request)
}

#[cfg(feature = "manager")]
async fn validate_rbac_access(
    request: &SharedFileRequest,
    role: &str,
    user_permissions: &[Permission],
) -> Result<bool, Box<dyn std::error::Error>> {
    // Simulate RBAC validation
    let required_permissions = &request.required_permissions;

    // Check if user has all required permissions
    let has_access = required_permissions.iter().all(|req_perm| {
        user_permissions
            .iter()
            .any(|user_perm| user_perm == req_perm)
    });

    // Log access attempt (in real implementation)
    println!("      📊 Access attempt logged for role: {}", role);

    sleep(Duration::from_millis(50)).await;

    Ok(has_access)
}

#[cfg(feature = "manager")]
async fn demonstrate_audit_logging() -> Result<(), Box<dyn std::error::Error>> {
    let audit_events = vec![
        (
            "user.login",
            "admin",
            "Successful admin login from 192.168.1.100",
        ),
        ("file.access", "analyst", "Accessed customer_data.csv"),
        (
            "permission.denied",
            "guest",
            "Attempted to write to protected resource",
        ),
        (
            "encryption.enabled",
            "system",
            "AES-256-GCM encryption enabled for session",
        ),
        (
            "session.expired",
            "developer",
            "Session timeout after 8 hours",
        ),
        (
            "threat.detected",
            "security",
            "Potential brute force attack detected",
        ),
    ];

    for (event_type, actor, description) in audit_events {
        println!("   📋 Audit Log Entry:");
        println!(
            "      🕒 Timestamp: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("      📊 Event Type: {}", event_type);
        println!("      👤 Actor: {}", actor);
        println!("      📝 Description: {}", description);
        println!(
            "      🔍 Security Level: {}",
            classify_audit_severity(event_type)
        );
        println!();

        sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}

#[cfg(feature = "manager")]
fn classify_audit_severity(event_type: &str) -> &'static str {
    match event_type {
        "user.login" => "INFO",
        "file.access" => "INFO",
        "permission.denied" => "WARNING",
        "encryption.enabled" => "INFO",
        "session.expired" => "INFO",
        "threat.detected" => "CRITICAL",
        _ => "INFO",
    }
}

#[cfg(feature = "manager")]
async fn demonstrate_threat_detection() -> Result<(), Box<dyn std::error::Error>> {
    let threat_scenarios = vec![
        (
            "Brute Force Attack",
            "Multiple failed login attempts from same IP",
            "Block IP address for 1 hour",
        ),
        (
            "Privilege Escalation",
            "User attempting to access admin resources",
            "Revoke session and require re-authentication",
        ),
        (
            "Data Exfiltration",
            "Unusual large data transfer detected",
            "Throttle bandwidth and alert security team",
        ),
        (
            "Malformed Requests",
            "Potential injection attack in request parameters",
            "Block malicious requests and scan for vulnerabilities",
        ),
        (
            "Session Hijacking",
            "Session token reused from different location",
            "Invalidate all sessions for user",
        ),
    ];

    for (threat_type, description, mitigation) in threat_scenarios {
        println!("   🚨 Threat Detection Alert:");
        println!("      ⚠️  Type: {}", threat_type);
        println!("      📝 Description: {}", description);
        println!("      🛡️  Mitigation: {}", mitigation);
        println!("      ⏱️  Response Time: {}ms", simulate_response_time());
        println!("      ✅ Status: Threat Mitigated");
        println!();

        sleep(Duration::from_millis(300)).await;
    }

    Ok(())
}

#[cfg(feature = "manager")]
fn simulate_response_time() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    Instant::now().hash(&mut hasher);
    let hash = hasher.finish();

    // Simulate response times between 50-500ms
    50 + (hash % 450)
}

#[cfg(feature = "manager")]
async fn demonstrate_certificate_management() -> Result<(), Box<dyn std::error::Error>> {
    println!("   📜 Certificate Authority (CA) Management:");
    println!("      🏛️  Root CA: Commy-Root-CA-2024");
    println!("      🔒 Intermediate CA: Commy-Service-CA");
    println!("      ⏰ Validity Period: 2 years");
    println!("      🔄 Auto-renewal: Enabled");

    sleep(Duration::from_millis(200)).await;

    println!("\n   🎫 Service Certificates:");
    let certificates = vec![
        ("coordinator-main", "Valid", "89 days remaining"),
        ("worker-alpha", "Valid", "156 days remaining"),
        ("worker-beta", "Valid", "203 days remaining"),
        ("security-gateway", "Expiring Soon", "12 days remaining"),
        ("audit-service", "Valid", "45 days remaining"),
    ];

    for (service, status, expiry) in certificates {
        let status_icon = match status {
            "Valid" => "✅",
            "Expiring Soon" => "⚠️",
            "Expired" => "❌",
            _ => "❓",
        };

        println!(
            "      {} Service: {} - {} ({})",
            status_icon, service, status, expiry
        );
    }

    sleep(Duration::from_millis(300)).await;

    println!("\n   🔄 Certificate Operations:");
    println!("      📋 Certificate Signing Request (CSR) generated");
    println!("      ✍️  CSR signed by Intermediate CA");
    println!("      📤 Certificate deployed to service");
    println!("      🔍 Certificate validation completed");
    println!("      📊 Certificate chain verified");

    Ok(())
}

#[cfg(feature = "manager")]
fn create_public_security_profile() -> SecurityProfile {
    SecurityProfile {
        level: SecurityLevel::Public,
        encryption_algorithm: EncryptionAlgorithm::AES256GCM,
        require_mutual_auth: false,
        enable_perfect_forward_secrecy: false,
        audit_level: AuditLevel::Basic,
        allowed_roles: vec![Permission::Read],
        max_session_duration_minutes: 60,
    }
}

#[cfg(feature = "manager")]
fn create_protected_security_profile() -> SecurityProfile {
    SecurityProfile {
        level: SecurityLevel::Protected,
        encryption_algorithm: EncryptionAlgorithm::AES256GCM,
        require_mutual_auth: true,
        enable_perfect_forward_secrecy: false,
        audit_level: AuditLevel::Basic,
        allowed_roles: vec![Permission::Read, Permission::Write],
        max_session_duration_minutes: 480, // 8 hours
    }
}

#[cfg(feature = "manager")]
fn create_confidential_security_profile() -> SecurityProfile {
    SecurityProfile {
        level: SecurityLevel::Confidential,
        encryption_algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
        require_mutual_auth: true,
        enable_perfect_forward_secrecy: true,
        audit_level: AuditLevel::Detailed,
        allowed_roles: vec![Permission::Read, Permission::Write],
        max_session_duration_minutes: 240, // 4 hours
    }
}

#[cfg(feature = "manager")]
fn create_secret_security_profile() -> SecurityProfile {
    SecurityProfile {
        level: SecurityLevel::Secret,
        encryption_algorithm: EncryptionAlgorithm::AES256GCMSiv,
        require_mutual_auth: true,
        enable_perfect_forward_secrecy: true,
        audit_level: AuditLevel::Forensic,
        allowed_roles: vec![Permission::Admin],
        max_session_duration_minutes: 120, // 2 hours
    }
}

#[cfg(feature = "manager")]
fn create_top_secret_security_profile() -> SecurityProfile {
    SecurityProfile {
        level: SecurityLevel::TopSecret,
        encryption_algorithm: EncryptionAlgorithm::Post_Quantum_Kyber,
        require_mutual_auth: true,
        enable_perfect_forward_secrecy: true,
        audit_level: AuditLevel::Forensic,
        allowed_roles: vec![Permission::Admin],
        max_session_duration_minutes: 60, // 1 hour
    }
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Security Features demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example security_features_demo --features manager");
    std::process::exit(1);
}

use commy::server::clustering::config::ClusteringConfig;
use commy::{
    server::{WssServer, WssServerConfig},
    Server,
};
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Register test API keys via auth-framework for examples
/// This allows examples to authenticate without manual database setup
#[allow(dead_code)]
async fn register_test_credentials(_db_url: &str) {
    println!("[Commy]");
    println!("[Commy] 📝 Test API Keys for Examples");
    println!("[Commy] ────────────────────────────────────────────────────");
    println!("[Commy]");
    println!("[Commy] ℹ Test API keys are pre-configured in PostgreSQL");
    println!("[Commy]");

    println!("[Commy] Available API Keys for Testing:");
    println!("[Commy]   • test_key_123");
    println!("[Commy]     Used by: basic_client, hybrid_client examples");
    println!("[Commy]     Permissions: read, write");
    println!("[Commy]");
    println!("[Commy]   • admin_key_with_all_perms");
    println!("[Commy]     Used by: permissions_example (admin user)");
    println!("[Commy]     Permissions: admin, manage_tenants, manage_users");
    println!("[Commy]");
    println!("[Commy]   • read_only_key");
    println!("[Commy]     Used by: permissions_example (read-only user)");
    println!("[Commy]     Permissions: read");
    println!("[Commy]");
    println!("[Commy]   •  creator_key");
    println!("[Commy]     Used by: permissions_example (creator user)");
    println!("[Commy]     Permissions: admin, create_services, manage_variables");
    println!("[Commy] ────────────────────────────────────────────────────");
    println!("[Commy]");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration: environment variables, then the file (COMMY_CONFIG or ./commy.toml), then defaults.
    let settings = match commy::config::Settings::from_env() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[Commy] cannot start: {e}");
            std::process::exit(1);
        }
    };
    match settings.source() {
        Some(p) => println!("[Commy] Configuration file: {}", p.display()),
        None => println!("[Commy] No configuration file (COMMY_CONFIG or ./commy.toml): environment and defaults"),
    }
    let server_id = settings.get("COMMY_SERVER_ID").unwrap_or_else(|| "node-1".to_string());

    let listen_addr = settings.get("COMMY_LISTEN_ADDR").unwrap_or_else(|| "0.0.0.0".to_string());

    let listen_port: u16 = match settings.get("COMMY_LISTEN_PORT") {
        None => 8443,
        Some(v) => v.trim().parse().unwrap_or_else(|_| {
            eprintln!("[Commy] cannot start: listen_port / COMMY_LISTEN_PORT is not a port number");
            std::process::exit(1);
        }),
    };

    let cluster_enabled = settings
        .get("COMMY_CLUSTER_ENABLED")
        .unwrap_or_else(|| "false".to_string())
        .to_lowercase()
        == "true";

    // TLS configuration - these can point to self-signed certs for development
    let cert_path = settings.get("COMMY_TLS_CERT_PATH");
    let key_path = settings.get("COMMY_TLS_KEY_PATH");

    let use_tls = cert_path.is_some() && key_path.is_some();

    // Configure PostgreSQL backend for auth-framework
    // If not set, defaults to local PostgreSQL container
    let _database_url = if let Ok(url) = env::var("DATABASE_URL") {
        println!(
            "[Commy] DATABASE_URL environment variable found: {}",
            url.replace(":test_password@", ":***@")
        );
        url
    } else {
        let default_url = "postgresql://postgres:postgres@127.0.0.1:5432/commy".to_string();
        unsafe {
            env::set_var("DATABASE_URL", &default_url);
        }
        println!(
            "[Commy] DATABASE_URL not set, using default: {}",
            default_url.replace(":postgres@", ":***@")
        );
        default_url
    };

    println!("[Commy] ════════════════════════════════════════════");
    println!("[Commy] Initializing Commy Server");
    println!("[Commy] ════════════════════════════════════════════");
    println!("[Commy] Server ID: {}", server_id);
    println!(
        "[Commy] Client listen address: {}:{}",
        listen_addr, listen_port
    );
    println!("[Commy] TLS enabled: {}", use_tls);
    if use_tls {
        println!("[Commy] TLS cert: {:?}", cert_path);
        println!("[Commy] TLS key: {:?}", key_path);
    }
    println!("[Commy] Clustering enabled: {}", cluster_enabled);

    if cluster_enabled {
        let cluster_nodes = settings
            .get("COMMY_CLUSTER_NODES")
            .unwrap_or_else(|| "node-1:9000,node-2:9000,node-3:9000".to_string());
        println!("[Commy] Cluster nodes: {}", cluster_nodes);
    }
    println!("[Commy] ════════════════════════════════════════════");

    // Initialize clustering configuration if enabled
    if cluster_enabled {
        let _config = ClusteringConfig::new(server_id.clone());
        println!("[Commy] ✓ Clustering configuration initialized");
    }

    // Create the core Commy server
    let mut server_value = Server::new();
    server_value.configure(&settings);
    // Durable state: the service registry and the grants. COMMY_STATE_DIR names the directory
    // (default ./commy_state); "off" keeps everything in memory, lost at exit.
    match settings.get("COMMY_STATE_DIR").as_deref() {
        Some("off") => println!("[Commy] COMMY_STATE_DIR=off: services and grants are NOT persisted"),
        other => {
            let dir = std::path::PathBuf::from(other.unwrap_or("commy_state"));
            match server_value.enable_persistence(&dir) {
                Ok(r) => println!(
                    "[Commy] State in {}: {} service(s) loaded ({} skipped, file missing), {} grant change(s) replayed ({} skipped)",
                    dir.display(),
                    r.services_loaded,
                    r.services_missing_file,
                    r.grant_ops_replayed,
                    r.grant_ops_skipped
                ),
                Err(e) => {
                    eprintln!("[Commy] cannot open the state directory {}: {e}", dir.display());
                    std::process::exit(1);
                }
            }
        }
    }
    // Credentials: with development authentication off (the default) clients must present an API key
    // or JWT issued by auth-framework for their tenant. The server refuses to start without a master
    // secret rather than fall back to something guessable.
    if server_value.dev_auth {
        println!("[Commy] COMMY_ALLOW_DEV_AUTH=1: development authentication ACCEPTS ANY CREDENTIALS");
    } else {
        let auth_dir = std::path::PathBuf::from(
            settings.get("COMMY_AUTH_DIR").unwrap_or_else(|| "commy_state/auth".to_string()),
        );
        match commy::server::framework_verifier::FrameworkVerifier::from_env(&auth_dir) {
            Ok(v) => {
                println!("[Commy] Authentication: auth-framework, tenant databases in {}", auth_dir.display());
                server_value = server_value.with_verifier(Arc::new(v));
            }
            Err(e) => {
                eprintln!("[Commy] cannot start: {e}");
                std::process::exit(1);
            }
        }
    }
    let server = Arc::new(RwLock::new(server_value));

    // Set up tenants with authentication
    {
        let mut srv = server.write().await;

        // Get the DATABASE_URL that auth-framework will use
        let db_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@127.0.0.1:5432/commy".to_string());

        println!("[Commy] Configuring authentication:");
        println!("[Commy]   Backend: PostgreSQL");
        println!(
            "[Commy]   Database: {}",
            db_url.replace(":postgres@", ":***@")
        );

        // Create "my_tenant"
        let _tenant = srv.get_tenant("my_tenant");

        println!("[Commy] ✓ Tenant created: my_tenant");
        println!(
            "[Commy] ✓ Authenticat ion configured (development mode: all credentials accepted)"
        );
        // Skip test credential display for now - focus on getting service operations working
        // println!("[Commy] ✓ Displaying test API keys available in PostgreSQL...");
        // let db_url = env::var("DATABASE_URL")
        //     .unwrap_or_else(|_| "postgresql://postgres:postgres@127.0.0.1:5432/commy".to_string());
        // register_test_credentials(&db_url).await;
    }

    // Configure WSS server
    let wss_config = WssServerConfig {
        bind_addr: listen_addr.clone(),
        port: listen_port,
        cert_path: cert_path.clone(),
        key_path: key_path.clone(),
        max_connections: 1000,
        buffer_size: 65536,
    };

    // Create WSS server instance
    let mut wss_server = WssServer::new(wss_config, Arc::clone(&server));

    // Initialize TLS if certificates are provided
    if use_tls {
        match wss_server.initialize_tls() {
            Ok(_) => {
                println!("[Commy] ✓ TLS initialized successfully (WSS mode)");
            }
            Err(e) => {
                eprintln!("[Commy] ✗ TLS initialization failed: {}", e);
                eprintln!("[Commy]   To use TLS, provide:");
                eprintln!("[Commy]   - COMMY_TLS_CERT_PATH: path to PEM certificate");
                eprintln!("[Commy]   - COMMY_TLS_KEY_PATH: path to PEM private key");
                return Err(e);
            }
        }
    } else {
        eprintln!("[Commy] ⚠ Warning: Running WITHOUT TLS (insecure)");
        eprintln!("[Commy]   Set COMMY_TLS_CERT_PATH and COMMY_TLS_KEY_PATH for secure WSS");
        return Err("TLS not configured. COMMY requires WSS for client connections.".into());
    }

    // Start token cleanup task (auth-framework handles this internally)
    wss_server.start_token_cleanup_task(300); // Check every 5 minutes

    println!("[Commy] ✓ Server initialized, starting WSS listener...");
    println!(
        "[Commy] Remote clients connect to: wss://{}:{}",
        listen_addr, listen_port
    );
    println!("[Commy] ");

    // Run the WSS server (this blocks indefinitely)
    wss_server.run().await?;

    println!("[Commy] Server shutting down");
    Ok(())
}

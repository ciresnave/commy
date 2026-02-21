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
    // Parse environment variables for configuration
    let server_id = env::var("COMMY_SERVER_ID").unwrap_or_else(|_| "node-1".to_string());

    let listen_addr = env::var("COMMY_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8000".to_string());

    let listen_port: u16 = env::var("COMMY_LISTEN_PORT")
        .unwrap_or_else(|_| "8443".to_string())
        .parse()
        .expect("Invalid COMMY_LISTEN_PORT");

    let cluster_enabled = env::var("COMMY_CLUSTER_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    // TLS configuration - these can point to self-signed certs for development
    let cert_path = env::var("COMMY_TLS_CERT_PATH").ok();
    let key_path = env::var("COMMY_TLS_KEY_PATH").ok();

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
        let cluster_nodes = env::var("COMMY_CLUSTER_NODES")
            .unwrap_or_else(|_| "node-1:9000,node-2:9000,node-3:9000".to_string());
        println!("[Commy] Cluster nodes: {}", cluster_nodes);
    }
    println!("[Commy] ════════════════════════════════════════════");

    // Initialize clustering configuration if enabled
    if cluster_enabled {
        let _config = ClusteringConfig::new(server_id.clone());
        println!("[Commy] ✓ Clustering configuration initialized");
    }

    // Create the core Commy server
    let server = Arc::new(RwLock::new(Server::new()));

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

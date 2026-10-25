// SPDX-License-Identifier: MIT
//! Configuration file (plan R7).
//!
//! Settings come from, in order of precedence: the environment variable `COMMY_<NAME>`, the
//! configuration file, the built-in default. The file is TOML with flat keys named like the variables
//! without the `COMMY_` prefix, lower case (`listen_port = 8443` is `COMMY_LISTEN_PORT`). It is read
//! from the path in `COMMY_CONFIG`, else `./commy.toml` if that exists.
//!
//! The file never holds secrets or switches that make the server insecure: `auth_master_secret`,
//! `database_url` and `allow_dev_auth` are refused with an explanation, and so is any key not listed
//! in [`KEYS`] (a misspelt key must fail at start-up, not be ignored).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The environment variable naming the configuration file.
pub const CONFIG_ENV: &str = "COMMY_CONFIG";

/// The file read when `COMMY_CONFIG` is not set and the file exists.
pub const DEFAULT_CONFIG_FILE: &str = "commy.toml";

/// Every key the file may hold (the environment variable is `COMMY_` plus the upper-case key).
pub const KEYS: &[&str] = &[
    "server_id",
    "listen_addr",
    "listen_port",
    "cluster_enabled",
    "cluster_nodes",
    "tls_cert_path",
    "tls_key_path",
    "state_dir",
    "auth_dir",
    "max_service_size",
    "drain_timeout_ms",
    "fill_timeout_ms",
    "audit_max_events",
    "grant_compact_after",
    "rotation_min_interval_ms",
];

/// The keys a running server applies when the file changes (R26); every other key is read once, at
/// start-up, and a change to it waits for a restart.
pub const LIVE_KEYS: &[&str] = &[
    "fill_timeout_ms",
    "drain_timeout_ms",
    "rotation_min_interval_ms",
    "max_service_size",
    "audit_max_events",
    "grant_compact_after",
];

/// How often [`watch`] looks at the file.
pub const RELOAD_POLL: std::time::Duration = std::time::Duration::from_secs(5);

/// Watch the configuration file at `path` and apply a change to the [`LIVE_KEYS`] (plan R26):
/// poll its modification time and size every `every`; on a change parse the whole file, and if it
/// is not valid (syntax, unknown or forbidden key, unreadable) keep the current settings and record
/// why. `current` is what the server started with. A change to a key outside [`LIVE_KEYS`] is
/// recorded as waiting for a restart. Every outcome is an audit event, never a value of a secret
/// (the file holds none). The environment still beats the file, as at start-up.
pub fn watch(
    server: std::sync::Arc<tokio::sync::RwLock<crate::Server>>,
    path: PathBuf,
    mut current: Settings,
    every: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let signature = |p: &Path| std::fs::metadata(p).ok().map(|m| (m.modified().ok(), m.len()));
        // None until the first look, so a change made between reading at start-up and now is seen.
        let mut seen: Option<Option<(Option<std::time::SystemTime>, u64)>> = None;
        loop {
            let now = signature(&path);
            if seen.as_ref() != Some(&now) {
                seen = Some(now);
                match Settings::load(&path) {
                    Err(e) => {
                        server.read().await.audit(&format!("config: reload rejected, keeping the current settings: {e}"));
                    }
                    Ok(new) => {
                        let mut s = server.write().await;
                        for key in KEYS.iter().filter(|k| !LIVE_KEYS.contains(k)) {
                            if current.file_value(key) != new.file_value(key) {
                                s.audit(&format!("config: {key} changed in the file; it takes effect after a restart"));
                            }
                        }
                        s.apply_live_settings(&new);
                        drop(s);
                        current = new;
                    }
                }
            }
            tokio::time::sleep(every).await;
        }
    })
}

/// Keys that are refused in the file, with the reason.
const FORBIDDEN: &[(&str, &str)] = &[
    ("auth_master_secret", "secrets belong in the environment (COMMY_AUTH_MASTER_SECRET), never in a file that gets copied and committed"),
    ("database_url", "it can carry a password; set DATABASE_URL in the environment"),
    ("allow_dev_auth", "development authentication accepts any credential and must be switched on deliberately, in the environment, every time"),
];

/// Why the configuration could not be loaded.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// The file named by `COMMY_CONFIG` (or found at the default path) could not be read.
    Unreadable(String),
    /// The file is not valid TOML.
    Syntax(String),
    /// A key that is not a setting.
    UnknownKey(String),
    /// A key that must not be in a file.
    Forbidden(String, &'static str),
    /// A value that is not a string, integer or boolean.
    BadValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Unreadable(e) => write!(f, "cannot read the configuration file: {e}"),
            ConfigError::Syntax(e) => write!(f, "the configuration file is not valid TOML: {e}"),
            ConfigError::UnknownKey(k) => {
                write!(f, "unknown setting `{k}` (known: {})", KEYS.join(", "))
            }
            ConfigError::Forbidden(k, why) => write!(f, "`{k}` must not be in the configuration file: {why}"),
            ConfigError::BadValue(k) => write!(f, "`{k}` must be a string, an integer or true/false"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The settings read from the configuration file, consulted behind the environment.
#[derive(Debug, Default, Clone)]
pub struct Settings {
    file: HashMap<String, String>,
    source: Option<PathBuf>,
}

impl Settings {
    /// No file: only the environment and the defaults.
    pub fn empty() -> Self {
        Settings::default()
    }

    /// Parse the text of a configuration file.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| ConfigError::Syntax(e.message().to_string()))?;
        let mut file = HashMap::new();
        for (key, value) in table {
            if let Some((_, why)) = FORBIDDEN.iter().find(|(k, _)| *k == key) {
                return Err(ConfigError::Forbidden(key, why));
            }
            if !KEYS.contains(&key.as_str()) {
                return Err(ConfigError::UnknownKey(key));
            }
            let text = match value {
                toml::Value::String(s) => s,
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Boolean(b) => b.to_string(),
                _ => return Err(ConfigError::BadValue(key)),
            };
            file.insert(key, text);
        }
        Ok(Settings { file, source: None })
    }

    /// Read and parse the file at `path`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Unreadable(format!("{}: {e}", path.display())))?;
        let mut s = Self::parse(&text)?;
        s.source = Some(path.to_path_buf());
        Ok(s)
    }

    /// The file named by `COMMY_CONFIG` (it must exist), else `./commy.toml` if present, else nothing.
    pub fn from_env() -> Result<Self, ConfigError> {
        match std::env::var(CONFIG_ENV) {
            Ok(p) if !p.is_empty() => Self::load(Path::new(&p)),
            _ if Path::new(DEFAULT_CONFIG_FILE).is_file() => Self::load(Path::new(DEFAULT_CONFIG_FILE)),
            _ => Ok(Self::empty()),
        }
    }

    /// What the file says for `key` (lower case, no `COMMY_`), ignoring the environment.
    pub fn file_value(&self, key: &str) -> Option<&str> {
        self.file.get(key).map(String::as_str)
    }

    /// The file the settings came from, if any.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// The value of `COMMY_<NAME>` (pass the full variable name): the environment wins, then the file.
    pub fn get(&self, var: &str) -> Option<String> {
        if let Ok(v) = std::env::var(var) {
            return Some(v);
        }
        let key = var.strip_prefix("COMMY_")?.to_ascii_lowercase();
        self.file.get(&key).cloned()
    }

    /// [`Settings::get`] parsed; a value that does not parse is `None`, like an unset one.
    pub fn get_parsed<T: std::str::FromStr>(&self, var: &str) -> Option<T> {
        self.get(var).and_then(|v| v.trim().parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_supplies_values_and_the_environment_overrides_it() {
        let s = Settings::parse("listen_port = 9000\nserver_id = \"n7\"\ncluster_enabled = true\n").unwrap();
        assert_eq!(s.get("COMMY_SERVER_ID_CFG_TEST_ABSENT"), None, "an unknown name is absent");
        assert_eq!(s.get_parsed::<u16>("COMMY_LISTEN_PORT"), Some(9000));
        assert_eq!(s.get("COMMY_SERVER_ID").as_deref(), Some("n7"));
        assert_eq!(s.get("COMMY_CLUSTER_ENABLED").as_deref(), Some("true"));
        // the environment wins (a variable no other test touches)
        std::env::set_var("COMMY_TLS_KEY_PATH", "from-env");
        let s = Settings::parse("tls_key_path = \"from-file\"
").unwrap();
        assert_eq!(s.get("COMMY_TLS_KEY_PATH").as_deref(), Some("from-env"));
        std::env::remove_var("COMMY_TLS_KEY_PATH");
        assert_eq!(s.get("COMMY_TLS_KEY_PATH").as_deref(), Some("from-file"), "control: without the variable the file answers");
    }

    #[test]
    fn a_misspelt_or_unknown_key_is_an_error_not_ignored() {
        assert_eq!(Settings::parse("listn_port = 1\n").err(), Some(ConfigError::UnknownKey("listn_port".into())));
    }

    #[test]
    fn secrets_and_the_dev_auth_switch_are_refused_in_a_file() {
        for k in ["auth_master_secret", "database_url", "allow_dev_auth"] {
            let r = Settings::parse(&format!("{k} = \"x\"\n"));
            assert!(matches!(r, Err(ConfigError::Forbidden(ref key, _)) if key == k), "{k}: {r:?}");
        }
        let msg = Settings::parse("auth_master_secret = \"hunter2hunter2\"\n").err().unwrap().to_string();
        assert!(!msg.contains("hunter2"), "the error must not echo the value: {msg}");
    }

    #[test]
    fn bad_syntax_and_bad_value_types_are_errors() {
        assert!(matches!(Settings::parse("listen_port = = 1"), Err(ConfigError::Syntax(_))));
        assert_eq!(Settings::parse("cluster_nodes = [\"a\"]\n").err(), Some(ConfigError::BadValue("cluster_nodes".into())));
    }

    #[test]
    fn a_missing_named_file_is_an_error_and_a_real_file_loads() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(Settings::load(&d.path().join("nope.toml")), Err(ConfigError::Unreadable(_))));
        let p = d.path().join("commy.toml");
        std::fs::write(&p, "max_service_size = 4096\n").unwrap();
        let s = Settings::load(&p).unwrap();
        assert_eq!(s.get_parsed::<u64>("COMMY_MAX_SERVICE_SIZE"), Some(4096));
        assert_eq!(s.source(), Some(p.as_path()));
    }

    #[test]
    fn every_documented_key_is_accepted() {
        let all: String = KEYS.iter().map(|k| format!("{k} = \"1\"\n")).collect();
        assert!(Settings::parse(&all).is_ok());
    }

    #[test]
    fn configure_applies_the_file_to_the_server_and_leaves_defaults_alone() {
        let defaults = crate::Server::new();
        let mut s = crate::Server::new();
        s.configure(&Settings::parse("max_service_size = 4096
fill_timeout_ms = 111
drain_timeout_ms = 222
audit_max_events = 7
rotation_min_interval_ms = 2500
").unwrap());
        assert_eq!(s.max_service_size, 4096);
        assert_eq!(s.fill_timeout, std::time::Duration::from_millis(111));
        assert_eq!(s.drain_timeout, std::time::Duration::from_millis(222));
        assert_eq!(s.audit_max_events, Some(7));
        assert_eq!(s.rotation_min_interval, std::time::Duration::from_millis(2500));
        assert_eq!(crate::Server::new().rotation_min_interval, crate::server::rotation::DEFAULT_MIN_INTERVAL);
        let mut t = crate::Server::new();
        t.configure(&Settings::empty());
        assert_eq!((t.max_service_size, t.fill_timeout, t.drain_timeout), (defaults.max_service_size, defaults.fill_timeout, defaults.drain_timeout));
    }

    #[test]
    fn live_settings_apply_revert_to_defaults_and_are_audited() {
        let mut s = crate::Server::new();
        let d = crate::Server::new();
        let changes = s.apply_live_settings(&Settings::parse("fill_timeout_ms = 111
max_service_size = 4096
").unwrap());
        assert_eq!(s.fill_timeout, std::time::Duration::from_millis(111));
        assert_eq!(s.max_service_size, 4096);
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(s.audit_events().iter().any(|l| l == "config: fill_timeout_ms 30000 -> 111"));
        // a line removed from the file puts the default back
        s.apply_live_settings(&Settings::empty());
        assert_eq!((s.fill_timeout, s.max_service_size), (d.fill_timeout, d.max_service_size));
        // nothing changed, nothing audited
        assert!(s.apply_live_settings(&Settings::empty()).is_empty());
    }

    async fn until(mut ok: impl FnMut() -> bool) {
        for _ in 0..300 {
            if ok() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("timed out");
    }

    #[tokio::test]
    async fn the_watcher_applies_a_changed_file_rejects_a_bad_one_and_notes_restart_keys() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("commy.toml");
        std::fs::write(&path, "drain_timeout_ms = 100
").unwrap();
        let start = Settings::load(&path).unwrap();
        let server = Arc::new(tokio::sync::RwLock::new(crate::Server::new()));
        server.write().await.apply_live_settings(&start);
        let h = watch(Arc::clone(&server), path.clone(), start, std::time::Duration::from_millis(20));
        let drain = |s: &Arc<tokio::sync::RwLock<crate::Server>>| s.try_read().map(|g| g.drain_timeout.as_millis()).unwrap_or(0);
        let events = |s: &Arc<tokio::sync::RwLock<crate::Server>>| s.try_read().map(|g| g.audit_events()).unwrap_or_default();

        // a live key changes, and a restart-only key is noted but not applied
        std::fs::write(&path, "drain_timeout_ms = 200
listen_port = 9
").unwrap();
        until(|| drain(&server) == 200).await;
        until(|| events(&server).iter().any(|l| l.contains("listen_port changed in the file"))).await;

        // a file that does not parse changes nothing and says why; the length differs so it is seen
        std::fs::write(&path, "drain_timeout_ms = 300
no_such_key = 1
").unwrap();
        until(|| events(&server).iter().any(|l| l.contains("reload rejected") && l.contains("no_such_key"))).await;
        assert_eq!(drain(&server), 200, "the rejected file applied nothing, not even its valid line");

        // a forbidden key is rejected the same way
        std::fs::write(&path, "drain_timeout_ms = 400
database_url = \"x\"
").unwrap();
        until(|| events(&server).iter().any(|l| l.contains("reload rejected") && l.contains("database_url"))).await;
        assert_eq!(drain(&server), 200);

        // a good file after a bad one is applied; a deleted line returns to the default
        std::fs::write(&path, "fill_timeout_ms = 5
").unwrap();
        until(|| server.try_read().map(|g| g.fill_timeout.as_millis() == 5).unwrap_or(false)).await;
        assert_eq!(drain(&server), 5000, "drain_timeout_ms is gone from the file: default");
        h.abort();
    }
}

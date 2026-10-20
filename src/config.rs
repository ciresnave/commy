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
];

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
").unwrap());
        assert_eq!(s.max_service_size, 4096);
        assert_eq!(s.fill_timeout, std::time::Duration::from_millis(111));
        assert_eq!(s.drain_timeout, std::time::Duration::from_millis(222));
        assert_eq!(s.audit_max_events, Some(7));
        let mut t = crate::Server::new();
        t.configure(&Settings::empty());
        assert_eq!((t.max_service_size, t.fill_timeout, t.drain_timeout), (defaults.max_service_size, defaults.fill_timeout, defaults.drain_timeout));
    }
}

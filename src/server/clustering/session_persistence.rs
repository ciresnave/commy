use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Serializable client session state persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionData {
    pub session_id: String,
    pub client_id: String,
    pub tenant: String,
    pub services: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub last_activity: u64,
    pub version: u64,
    /// Opaque session payload (e.g., serialized client buffer)
    pub state: Vec<u8>,
}

impl SessionData {
    /// File name used for on-disk persistence
    pub fn file_name(&self) -> String {
        format!("session_{}.json", self.session_id)
    }
}

/// Simple file-backed session store. Uses JSON files per session and keeps an
/// in-memory cache for fast access.
pub struct SessionStore {
    dir: PathBuf,
    sessions: Arc<RwLock<HashMap<String, SessionData>>>,
}

impl SessionStore {
    /// Create a new store backed by `dir`. Loads any existing sessions.
    pub async fn new(dir: PathBuf) -> Result<Self, String> {
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        }

        let store = Self {
            dir,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        };

        store.load_all().await?;
        Ok(store)
    }

    /// Create and persist a session
    pub async fn create_session(&self, session: SessionData) -> Result<(), String> {
        let id = session.session_id.clone();
        self.persist_session(&session).await?;
        let mut s = self.sessions.write().await;
        s.insert(id, session);
        Ok(())
    }

    /// Replace/update a session
    pub async fn update_session(&self, session: SessionData) -> Result<(), String> {
        let id = session.session_id.clone();
        self.persist_session(&session).await?;
        let mut s = self.sessions.write().await;
        s.insert(id, session);
        Ok(())
    }

    /// Get a session by id
    pub async fn get_session(&self, session_id: &str) -> Option<SessionData> {
        let s = self.sessions.read().await;
        s.get(session_id).cloned()
    }

    /// Remove a session (memory + disk)
    pub async fn remove_session(&self, session_id: &str) -> Result<(), String> {
        let mut s = self.sessions.write().await;
        if s.remove(session_id).is_some() {
            let path = self.dir.join(format!("session_{}.json", session_id));
            if path.exists() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            Ok(())
        } else {
            Err("Session not found".to_string())
        }
    }

    /// Persist a single session to disk (blocking file IO executed via spawn_blocking)
    pub async fn persist_session(&self, session: &SessionData) -> Result<(), String> {
        let dir = self.dir.clone();
        let session_clone = session.clone();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            let path = dir.join(format!("session_{}.json", session_clone.session_id));
            let file = fs::File::create(&path).map_err(|e| e.to_string())?;
            serde_json::to_writer_pretty(file, &session_clone).map_err(|e| e.to_string())?;
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Load a single session from a file path
    pub async fn load_session_from_file(&self, path: PathBuf) -> Result<SessionData, String> {
        tokio::task::spawn_blocking(move || -> Result<SessionData, String> {
            let file = fs::File::open(&path).map_err(|e| e.to_string())?;
            let session: SessionData = serde_json::from_reader(file).map_err(|e| e.to_string())?;
            Ok(session)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Load all session files from the store directory into memory
    pub async fn load_all(&self) -> Result<(), String> {
        let dir = self.dir.clone();
        let mut loaded = HashMap::new();

        let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|s| s == "json").unwrap_or(false) {
                let session = self.load_session_from_file(path).await?;
                loaded.insert(session.session_id.clone(), session);
            }
        }

        let mut s = self.sessions.write().await;
        *s = loaded;
        Ok(())
    }

    /// List all sessions currently loaded in memory
    pub async fn list_all_sessions(&self) -> Vec<SessionData> {
        let s = self.sessions.read().await;
        s.values().cloned().collect()
    }

    /// List sessions that reference a particular service name
    pub async fn list_sessions_by_service(&self, service_name: &str) -> Vec<SessionData> {
        let s = self.sessions.read().await;
        s.values()
            .filter(|sess| sess.services.iter().any(|svc| svc == service_name))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_load_session() {
        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf()).await.unwrap();

        let mut metadata = HashMap::new();
        metadata.insert("role".to_string(), "reader".to_string());

        let session = SessionData {
            session_id: "s1".to_string(),
            client_id: "c1".to_string(),
            tenant: "t1".to_string(),
            services: vec!["svc1".to_string()],
            metadata,
            last_activity: 12345,
            version: 1,
            state: vec![1, 2, 3],
        };

        store.create_session(session.clone()).await.unwrap();

        // New store instance should load the persisted session
        let store2 = SessionStore::new(dir.path().to_path_buf()).await.unwrap();
        let loaded = store2.get_session("s1").await.unwrap();
        assert_eq!(loaded, session);
    }

    #[tokio::test]
    async fn test_update_and_remove_session() {
        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf()).await.unwrap();

        let session = SessionData {
            session_id: "s2".to_string(),
            client_id: "c2".to_string(),
            tenant: "t2".to_string(),
            services: vec!["svcA".to_string()],
            metadata: HashMap::new(),
            last_activity: 0,
            version: 1,
            state: vec![],
        };

        store.create_session(session.clone()).await.unwrap();

        let mut updated = session.clone();
        updated.version = 2;
        updated.state = vec![9, 8, 7];

        store.update_session(updated.clone()).await.unwrap();

        let loaded = store.get_session("s2").await.unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.state, vec![9, 8, 7]);

        store.remove_session("s2").await.unwrap();
        assert!(store.get_session("s2").await.is_none());
    }
}

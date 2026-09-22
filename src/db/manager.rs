//! Connection profile manager and active pool registry.

use crate::db::{
    error::{DbError, DbResult},
    handle::ActiveConnection,
    types::{ConnectionConfig, ConnectionStatus},
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages saved connection configurations and active database sessions.
pub struct ConnectionManager {
    /// In-memory registry of active connections by connection ID.
    active_connections: Arc<RwLock<HashMap<String, Arc<ActiveConnection>>>>,
    /// Path to the local JSON configuration file.
    config_file_path: PathBuf,
}

impl ConnectionManager {
    /// Creates a new connection manager with the default platform configuration directory.
    pub fn new() -> Self {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zqlcrab");
        let _ = fs::create_dir_all(&base_dir);
        let config_file_path = base_dir.join("connections.json");

        Self {
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            config_file_path,
        }
    }

    /// Ensure default friendly presets (Local Docker MySQL, Demo SQLite) exist in profile list.
    pub fn ensure_default_presets(&mut self) {
        let mut configs = self.load_saved_configs();
        let mut updated = false;

        // Add Local Docker MySQL preset if not present
        if !configs.iter().any(|c| c.name.contains("Docker MySQL")) {
            let mut mysql_cfg = ConnectionConfig::mysql(
                "Local Docker MySQL",
                "127.0.0.1",
                3306,
                "skill_up_web",
                "root",
                Some("skillup_local_test".to_string()),
            );
            mysql_cfg.environment = crate::db::types::EnvironmentTag::Development;
            configs.insert(0, mysql_cfg);
            updated = true;
        }

        // Add SQLite memory DB preset if not present
        if !configs
            .iter()
            .any(|c| c.db_type == crate::db::types::DatabaseType::Sqlite)
        {
            let sqlite_cfg = ConnectionConfig::sqlite("Sample SQLite (In-Memory)", ":memory:");
            configs.push(sqlite_cfg);
            updated = true;
        }

        if updated {
            let _ = self.save_all_configs(&configs);
        }
    }

    /// Save full configuration list to disk.
    pub fn save_all_configs(&self, configs: &[ConnectionConfig]) -> DbResult<()> {
        if let Some(parent) = self.config_file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(configs)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.config_file_path, json).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// Load all saved connection configurations from disk.
    pub fn load_saved_configs(&self) -> Vec<ConnectionConfig> {
        if !self.config_file_path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&self.config_file_path) {
            Ok(content) => {
                serde_json::from_str::<Vec<ConnectionConfig>>(&content).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        }
    }

    /// List all saved connection configuration profiles.
    pub fn list_configs(&self) -> Vec<ConnectionConfig> {
        self.load_saved_configs()
    }

    /// Retrieve a saved configuration profile by ID.
    pub fn get_config(&self, id: &str) -> Option<ConnectionConfig> {
        self.load_saved_configs().into_iter().find(|c| c.id == id)
    }

    /// Save or update a connection configuration profile to disk.
    pub fn save_config(&self, config: ConnectionConfig) -> DbResult<()> {
        let mut configs = self.load_saved_configs();
        if let Some(idx) = configs.iter().position(|c| c.id == config.id) {
            configs[idx] = config;
        } else {
            configs.push(config);
        }

        let json = serde_json::to_string_pretty(&configs)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.config_file_path, json).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// Delete a connection configuration profile.
    pub fn delete_config(&self, id: &str) -> DbResult<()> {
        let mut configs = self.load_saved_configs();
        configs.retain(|c| c.id != id);

        let json = serde_json::to_string_pretty(&configs)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.config_file_path, json).map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// Connect to a database using the provided configuration and register as active.
    pub async fn connect(&self, config: ConnectionConfig) -> DbResult<Arc<ActiveConnection>> {
        let id = config.id.clone();
        let conn = Arc::new(ActiveConnection::new(config));
        conn.connect().await?;

        let mut active = self.active_connections.write().await;
        active.insert(id, conn.clone());
        Ok(conn)
    }

    /// Disconnect an active connection by its ID.
    pub async fn disconnect(&self, id: &str) -> DbResult<()> {
        let conn_to_disconnect = {
            let mut active = self.active_connections.write().await;
            active.remove(id)
        };
        if let Some(conn) = conn_to_disconnect {
            conn.disconnect().await?;
        }
        Ok(())
    }

    /// Disconnect all active connections gracefully (e.g. on application exit).
    pub async fn disconnect_all(&self) {
        let conns = {
            let mut active = self.active_connections.write().await;
            active.drain().map(|(_, c)| c).collect::<Vec<_>>()
        };
        for conn in conns {
            let _ = conn.disconnect().await;
        }
    }

    /// Retrieve an active connection handle if connected.
    pub async fn get_active(&self, id: &str) -> Option<Arc<ActiveConnection>> {
        let active = self.active_connections.read().await;
        active.get(id).cloned()
    }

    /// Retrieve all currently active connection IDs and their handles.
    pub async fn list_active(&self) -> Vec<(String, Arc<ActiveConnection>)> {
        let active = self.active_connections.read().await;
        active.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Check if a connection profile is currently active.
    pub async fn is_active(&self, id: &str) -> bool {
        let active = self.active_connections.read().await;
        active.contains_key(id)
    }

    /// Test a connection configuration without registering it as active.
    pub async fn test_config(&self, config: ConnectionConfig) -> DbResult<ConnectionStatus> {
        let conn = ActiveConnection::new(config);
        conn.connect().await?;
        let status = conn.test_connection().await?;
        let _ = conn.disconnect().await;
        Ok(status)
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_connection_lifecycle() {
        let manager = ConnectionManager::new();
        let config = ConnectionConfig::sqlite("Manager Test", ":memory:");
        let id = config.id.clone();

        // Connect
        let active_conn = manager
            .connect(config)
            .await
            .expect("connect should succeed");
        assert!(manager.is_active(&id).await);

        // Run query through active connection
        let res = active_conn
            .execute_query("SELECT 42 AS num;")
            .await
            .expect("query should succeed");
        assert_eq!(res.columns, vec!["num"]);

        // Disconnect
        manager
            .disconnect(&id)
            .await
            .expect("disconnect should succeed");
        assert!(!manager.is_active(&id).await);
    }

    #[tokio::test]
    async fn test_manager_disconnect_all() {
        let manager = ConnectionManager::new();
        let cfg1 = ConnectionConfig::sqlite("Test 1", ":memory:");
        let cfg2 = ConnectionConfig::sqlite("Test 2", ":memory:");
        let id1 = cfg1.id.clone();
        let id2 = cfg2.id.clone();

        manager.connect(cfg1).await.expect("connect 1");
        manager.connect(cfg2).await.expect("connect 2");
        assert_eq!(manager.list_active().await.len(), 2);

        manager.disconnect_all().await;
        assert_eq!(manager.list_active().await.len(), 0);
        assert!(!manager.is_active(&id1).await);
        assert!(!manager.is_active(&id2).await);
    }
}

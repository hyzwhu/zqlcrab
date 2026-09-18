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

        let manager = Self {
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            config_file_path,
        };

        // Initialize with default demo configuration if empty
        if manager.load_saved_configs().is_empty() {
            let demo_sqlite = ConnectionConfig::sqlite("Demo Memory DB", ":memory:");
            let _ = manager.save_config(demo_sqlite);
        }

        manager
    }

    /// Load all saved connection configurations from disk.
    pub fn load_saved_configs(&self) -> Vec<ConnectionConfig> {
        if !self.config_file_path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&self.config_file_path) {
            Ok(content) => serde_json::from_str::<Vec<ConnectionConfig>>(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
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
        fs::write(&self.config_file_path, json)
            .map_err(|e| DbError::Io(e.to_string()))?;
        Ok(())
    }

    /// Delete a connection configuration profile.
    pub fn delete_config(&self, id: &str) -> DbResult<()> {
        let mut configs = self.load_saved_configs();
        configs.retain(|c| c.id != id);

        let json = serde_json::to_string_pretty(&configs)
            .map_err(|e| DbError::Configuration(e.to_string()))?;
        fs::write(&self.config_file_path, json)
            .map_err(|e| DbError::Io(e.to_string()))?;
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
        let mut active = self.active_connections.write().await;
        if let Some(conn) = active.remove(id) {
            conn.disconnect().await?;
        }
        Ok(())
    }

    /// Retrieve an active connection handle if connected.
    pub async fn get_active(&self, id: &str) -> Option<Arc<ActiveConnection>> {
        let active = self.active_connections.read().await;
        active.get(id).cloned()
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
        let active_conn = manager.connect(config).await.expect("connect should succeed");
        assert!(manager.is_active(&id).await);

        // Run query through active connection
        let res = active_conn
            .execute_query("SELECT 42 AS num;")
            .await
            .expect("query should succeed");
        assert_eq!(res.columns, vec!["num"]);

        // Disconnect
        manager.disconnect(&id).await.expect("disconnect should succeed");
        assert!(!manager.is_active(&id).await);
    }
}

//! Background Tokio runtime provider for async database drivers and I/O.
//!
//! GPUI uses its own executor and event loop which does not provide a Tokio reactor.
//! Database drivers such as `mysql_async` and `tokio-postgres` require a Tokio 1.x
//! reactor context for socket polling, timers, and background tasks.
//! This module provides a shared multi-thread Tokio runtime and bridging routines.

use crate::db::error::{DbError, DbResult};
use std::future::Future;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

static TOKIO_RT: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("zqlcrab-tokio-worker")
        .build()
        .expect("Failed to initialize background Tokio runtime for database operations")
});

/// Returns a reference to the global background Tokio runtime.
pub fn tokio_runtime() -> &'static Runtime {
    &TOKIO_RT
}

/// Execute an async future on the background Tokio runtime and await its result.
///
/// This safely bridges GPUI's async tasks with Tokio-dependent drivers (like MySQL and PostgreSQL)
/// ensuring a Tokio 1.x reactor is always active for socket I/O, timers, and background worker loops.
pub async fn run_on_tokio<F, T>(fut: F) -> DbResult<T>
where
    F: Future<Output = DbResult<T>> + Send + 'static,
    T: Send + 'static,
{
    TOKIO_RT
        .spawn(fut)
        .await
        .map_err(|e| DbError::connection(format!("Background database task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::handle::ActiveConnection;
    use crate::db::types::ConnectionConfig;

    #[test]
    fn test_tokio_runtime_accessible_from_plain_thread() {
        let handle = std::thread::spawn(|| {
            // This OS thread has NO ambient Tokio runtime
            assert!(tokio::runtime::Handle::try_current().is_err());

            // Using futures::executor::block_on or polling run_on_tokio should work cleanly
            let res = tokio_runtime().block_on(async { run_on_tokio(async { Ok(42) }).await });
            assert_eq!(res.unwrap(), 42);
        });

        handle.join().expect("thread join should succeed");
    }

    #[tokio::test]
    async fn test_active_connection_connect_docker_mysql() {
        let config = ConnectionConfig::mysql(
            "Local Docker MySQL",
            "127.0.0.1",
            3306,
            "skill_up_web",
            "root",
            Some("skillup_local_test".to_string()),
        );
        let res = ActiveConnection::connect_config(config).await;
        println!(
            "test_active_connection_connect_docker_mysql result: {:?}",
            res.is_ok()
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_mysql_connection_does_not_panic_without_ambient_reactor() {
        let handle = std::thread::spawn(|| {
            // Explicitly run from a non-Tokio thread
            assert!(tokio::runtime::Handle::try_current().is_err());

            // Connect to an offline port.
            // Before this fix, Pool::new inside connect_config panicked with:
            // "there is no reactor running, must be called from the context of a Tokio 1.x runtime"
            let cfg = ConnectionConfig::mysql(
                "Test Offline MySQL",
                "127.0.0.1",
                59999,
                "none",
                "root",
                None,
            );
            let res =
                tokio_runtime().block_on(async { ActiveConnection::connect_config(cfg).await });

            // Must return an Err cleanly instead of panicking/aborting
            assert!(res.is_err());
        });

        handle.join().expect("thread must not panic or abort");
    }
}

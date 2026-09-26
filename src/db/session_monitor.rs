//! Multi-dialect database processlist and active session monitoring engine.

use crate::db::error::{DbError, DbResult};
use crate::db::types::{DatabaseFamily, QueryResult, QueryValue};
use serde::{Deserialize, Serialize};

/// Type of process termination or cancellation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KillAction {
    /// Cancel current executing statement without dropping backend connection.
    CancelQuery,
    /// Forcefully terminate backend connection session.
    TerminateSession,
}

impl KillAction {
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::CancelQuery => "Cancel Query",
            Self::TerminateSession => "Terminate Connection",
        }
    }
}

/// Standardized representation of an active database process or connection session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub username: String,
    pub database: String,
    pub client_addr: String,
    pub state: String,
    pub duration_secs: u64,
    pub wait_event: String,
    pub query: String,
    pub is_current: bool,
}

impl SessionInfo {
    /// Helper to classify latency severity for visual badge rendering.
    pub fn latency_severity(&self) -> SessionLatencySeverity {
        if self.duration_secs >= 60 {
            SessionLatencySeverity::Critical
        } else if self.duration_secs >= 15 {
            SessionLatencySeverity::Warning
        } else if self.duration_secs >= 3 {
            SessionLatencySeverity::Notice
        } else {
            SessionLatencySeverity::Normal
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLatencySeverity {
    Normal,   // < 3s
    Notice,   // 3s - 15s
    Warning,  // 15s - 60s
    Critical, // >= 60s
}

/// Construct the dialect-specific SQL to fetch active sessions.
pub fn build_session_list_sql(dialect: DatabaseFamily) -> &'static str {
    match dialect {
        DatabaseFamily::Postgres => {
            "SELECT \
                pid::text as id, \
                COALESCE(usename, '') as username, \
                COALESCE(datname, '') as database, \
                COALESCE(client_addr::text, 'local') as client_addr, \
                COALESCE(state, 'unknown') as state, \
                COALESCE(ROUND(EXTRACT(EPOCH FROM (clock_timestamp() - query_start)))::bigint, 0) as duration_secs, \
                COALESCE(wait_event_type || ':' || wait_event, wait_event, '') as wait_event, \
                COALESCE(query, '') as query, \
                CASE WHEN pid = pg_backend_pid() THEN 1 ELSE 0 END as is_current \
            FROM pg_stat_activity \
            WHERE pid != pg_backend_pid() OR query NOT LIKE '%pg_stat_activity%' \
            ORDER BY duration_secs DESC;"
        }
        DatabaseFamily::MySql => {
            "SELECT \
                CAST(id AS CHAR) as id, \
                COALESCE(user, '') as username, \
                COALESCE(db, '') as database, \
                COALESCE(host, 'local') as client_addr, \
                COALESCE(command, '') as state, \
                COALESCE(time, 0) as duration_secs, \
                COALESCE(state, '') as wait_event, \
                COALESCE(info, '') as query, \
                CASE WHEN id = CONNECTION_ID() THEN 1 ELSE 0 END as is_current \
            FROM information_schema.PROCESSLIST \
            ORDER BY duration_secs DESC;"
        }
        DatabaseFamily::Sqlite => "PRAGMA database_list;",
    }
}

/// Parse `QueryResult` rows into `Vec<SessionInfo>`.
pub fn parse_session_list(dialect: DatabaseFamily, result: &QueryResult) -> Vec<SessionInfo> {
    match dialect {
        DatabaseFamily::Sqlite => {
            let mut db_name = "main".to_string();
            if let Some(row) = result.rows.first() {
                if row.len() >= 2 {
                    db_name = row[1].to_display_string();
                }
            }
            vec![SessionInfo {
                id: "1".to_string(),
                username: "local".to_string(),
                database: db_name,
                client_addr: "in-process".to_string(),
                state: "active".to_string(),
                duration_secs: 0,
                wait_event: "none".to_string(),
                query: "PRAGMA database_list".to_string(),
                is_current: true,
            }]
        }
        DatabaseFamily::Postgres | DatabaseFamily::MySql => {
            let col_map: std::collections::HashMap<String, usize> = result
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| (col.to_lowercase(), i))
                .collect();

            let get_val = |row: &[QueryValue], key: &str| -> String {
                if let Some(&idx) = col_map.get(key) {
                    if let Some(v) = row.get(idx) {
                        return v.to_display_string();
                    }
                }
                String::new()
            };

            let get_u64 = |row: &[QueryValue], key: &str| -> u64 {
                if let Some(&idx) = col_map.get(key) {
                    if let Some(v) = row.get(idx) {
                        match v {
                            QueryValue::Int(i) => return (*i).max(0) as u64,
                            QueryValue::Float(f) => return (*f).max(0.0) as u64,
                            QueryValue::String(s) => return s.parse::<u64>().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
                0
            };

            let get_bool = |row: &[QueryValue], key: &str| -> bool {
                if let Some(&idx) = col_map.get(key) {
                    if let Some(v) = row.get(idx) {
                        match v {
                            QueryValue::Bool(b) => return *b,
                            QueryValue::Int(i) => return *i != 0,
                            QueryValue::String(s) => {
                                return s == "1" || s.eq_ignore_ascii_case("true");
                            }
                            _ => {}
                        }
                    }
                }
                false
            };

            result
                .rows
                .iter()
                .map(|row| SessionInfo {
                    id: get_val(row, "id"),
                    username: get_val(row, "username"),
                    database: get_val(row, "database"),
                    client_addr: get_val(row, "client_addr"),
                    state: get_val(row, "state"),
                    duration_secs: get_u64(row, "duration_secs"),
                    wait_event: get_val(row, "wait_event"),
                    query: get_val(row, "query"),
                    is_current: get_bool(row, "is_current"),
                })
                .collect()
        }
    }
}

/// Construct the SQL statement to execute a kill or terminate action.
pub fn build_kill_sql(
    dialect: DatabaseFamily,
    session_id: &str,
    action: KillAction,
) -> DbResult<String> {
    let clean_id = session_id.trim();
    if clean_id.is_empty() || !clean_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(DbError::Validation(format!(
            "Invalid session ID: '{}'. Must be numeric.",
            session_id
        )));
    }

    match dialect {
        DatabaseFamily::Postgres => match action {
            KillAction::CancelQuery => Ok(format!("SELECT pg_cancel_backend({});", clean_id)),
            KillAction::TerminateSession => {
                Ok(format!("SELECT pg_terminate_backend({});", clean_id))
            }
        },
        DatabaseFamily::MySql => match action {
            KillAction::CancelQuery => Ok(format!("KILL QUERY {};", clean_id)),
            KillAction::TerminateSession => Ok(format!("KILL {};", clean_id)),
        },
        DatabaseFamily::Sqlite => Err(DbError::unsupported(
            "SQLite is an embedded in-process database and does not support process termination."
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_session_list_sql_dialects() {
        let pg_sql = build_session_list_sql(DatabaseFamily::Postgres);
        assert!(pg_sql.contains("pg_stat_activity"));

        let mysql_sql = build_session_list_sql(DatabaseFamily::MySql);
        assert!(mysql_sql.contains("information_schema.PROCESSLIST"));

        let sqlite_sql = build_session_list_sql(DatabaseFamily::Sqlite);
        assert!(sqlite_sql.contains("PRAGMA database_list"));
    }

    #[test]
    fn test_build_kill_sql_postgres_and_mysql() {
        assert_eq!(
            build_kill_sql(DatabaseFamily::Postgres, "1234", KillAction::CancelQuery).unwrap(),
            "SELECT pg_cancel_backend(1234);"
        );
        assert_eq!(
            build_kill_sql(
                DatabaseFamily::Postgres,
                "1234",
                KillAction::TerminateSession
            )
            .unwrap(),
            "SELECT pg_terminate_backend(1234);"
        );

        assert_eq!(
            build_kill_sql(DatabaseFamily::MySql, "5678", KillAction::CancelQuery).unwrap(),
            "KILL QUERY 5678;"
        );
        assert_eq!(
            build_kill_sql(DatabaseFamily::MySql, "5678", KillAction::TerminateSession).unwrap(),
            "KILL 5678;"
        );

        assert!(build_kill_sql(DatabaseFamily::Sqlite, "1", KillAction::CancelQuery).is_err());
        assert!(
            build_kill_sql(
                DatabaseFamily::Postgres,
                "invalid-pid",
                KillAction::CancelQuery
            )
            .is_err()
        );
    }

    #[test]
    fn test_parse_session_list_postgres() {
        let cols = vec![
            "id".to_string(),
            "username".to_string(),
            "database".to_string(),
            "client_addr".to_string(),
            "state".to_string(),
            "duration_secs".to_string(),
            "wait_event".to_string(),
            "query".to_string(),
            "is_current".to_string(),
        ];
        let types = vec!["text".to_string(); 9];
        let rows = vec![
            vec![
                QueryValue::String("1001".to_string()),
                QueryValue::String("postgres".to_string()),
                QueryValue::String("mydb".to_string()),
                QueryValue::String("127.0.0.1".to_string()),
                QueryValue::String("active".to_string()),
                QueryValue::Int(42),
                QueryValue::String("Lock:relation".to_string()),
                QueryValue::String("SELECT * FROM large_table".to_string()),
                QueryValue::Int(0),
            ],
            vec![
                QueryValue::String("1002".to_string()),
                QueryValue::String("app_user".to_string()),
                QueryValue::String("mydb".to_string()),
                QueryValue::String("192.168.1.100".to_string()),
                QueryValue::String("idle in transaction".to_string()),
                QueryValue::Int(120),
                QueryValue::String("Client:ClientRead".to_string()),
                QueryValue::String("UPDATE accounts SET balance = balance - 100".to_string()),
                QueryValue::Int(1),
            ],
        ];

        let result = QueryResult::rows(cols, types, rows);
        let sessions = parse_session_list(DatabaseFamily::Postgres, &result);

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "1001");
        assert_eq!(sessions[0].duration_secs, 42);
        assert_eq!(
            sessions[0].latency_severity(),
            SessionLatencySeverity::Warning
        );
        assert!(!sessions[0].is_current);

        assert_eq!(sessions[1].id, "1002");
        assert_eq!(sessions[1].duration_secs, 120);
        assert_eq!(
            sessions[1].latency_severity(),
            SessionLatencySeverity::Critical
        );
        assert!(sessions[1].is_current);
    }

    #[test]
    fn test_parse_session_list_mysql_and_sqlite() {
        let cols = vec![
            "id".to_string(),
            "username".to_string(),
            "database".to_string(),
            "client_addr".to_string(),
            "state".to_string(),
            "duration_secs".to_string(),
            "wait_event".to_string(),
            "query".to_string(),
            "is_current".to_string(),
        ];
        let types = vec!["text".to_string(); 9];
        let rows = vec![vec![
            QueryValue::String("88".to_string()),
            QueryValue::String("root".to_string()),
            QueryValue::String("production".to_string()),
            QueryValue::String("10.0.0.5:54321".to_string()),
            QueryValue::String("Query".to_string()),
            QueryValue::Int(2),
            QueryValue::String("Sending data".to_string()),
            QueryValue::String("SELECT count(*) FROM orders".to_string()),
            QueryValue::Int(0),
        ]];

        let mysql_res = QueryResult::rows(cols, types, rows);
        let mysql_sessions = parse_session_list(DatabaseFamily::MySql, &mysql_res);
        assert_eq!(mysql_sessions.len(), 1);
        assert_eq!(mysql_sessions[0].id, "88");
        assert_eq!(
            mysql_sessions[0].latency_severity(),
            SessionLatencySeverity::Normal
        );

        let sqlite_res = QueryResult::rows(
            vec!["seq".to_string(), "name".to_string(), "file".to_string()],
            vec!["int".to_string(), "text".to_string(), "text".to_string()],
            vec![vec![
                QueryValue::Int(0),
                QueryValue::String("main".to_string()),
                QueryValue::String("/tmp/test.db".to_string()),
            ]],
        );
        let sqlite_sessions = parse_session_list(DatabaseFamily::Sqlite, &sqlite_res);
        assert_eq!(sqlite_sessions.len(), 1);
        assert_eq!(sqlite_sessions[0].database, "main");
        assert!(sqlite_sessions[0].is_current);
    }
}

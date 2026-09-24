//! Mock data generation engine and multi-dialect batch seeding generator.

use crate::db::error::{DbError, DbResult};
use crate::db::handle::ActiveConnection;
use crate::db::types::{ColumnInfo, DatabaseFamily};
use std::time::Instant;

/// Supported generator strategies for individual columns.
#[derive(Debug, Clone, PartialEq)]
pub enum MockGeneratorType {
    /// Incremental integer sequence starting from `start` with `step`.
    AutoIncrement { start: i64, step: i64 },
    /// Random integer uniformly distributed in `[min, max]`.
    RandomInt { min: i64, max: i64 },
    /// Random floating point value with specified decimal precision.
    RandomFloat { min: f64, max: f64, decimals: u32 },
    /// Realistic full person name (First + Last).
    PersonName,
    /// Realistic email address.
    Email,
    /// Realistic phone number.
    PhoneNumber,
    /// Standard UUID v4 string.
    UuidV4,
    /// Random timestamp within the last `past_days`.
    DateTime { past_days: u32 },
    /// Random date within the last `past_days`.
    Date { past_days: u32 },
    /// Random boolean with probability `true_ratio` of being true.
    Boolean { true_ratio: f64 },
    /// Randomly chosen string from a list of options.
    EnumChoices { options: Vec<String> },
    /// Pseudo-latin sentence / words.
    LoremIpsum { words: u32 },
    /// Constant fixed value string.
    FixedValue { value: String },
    /// SQL NULL.
    Null,
    /// Exclude this column entirely from the INSERT statement.
    Ignored,
}

impl MockGeneratorType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AutoIncrement { .. } => "Auto Increment",
            Self::RandomInt { .. } => "Random Int",
            Self::RandomFloat { .. } => "Random Float",
            Self::PersonName => "Person Name",
            Self::Email => "Email",
            Self::PhoneNumber => "Phone Number",
            Self::UuidV4 => "UUID v4",
            Self::DateTime { .. } => "DateTime",
            Self::Date { .. } => "Date",
            Self::Boolean { .. } => "Boolean",
            Self::EnumChoices { .. } => "Enum / Options",
            Self::LoremIpsum { .. } => "Lorem Ipsum",
            Self::FixedValue { .. } => "Fixed Value",
            Self::Null => "NULL",
            Self::Ignored => "Skip (Ignore)",
        }
    }

    pub fn is_ignored(&self) -> bool {
        matches!(self, Self::Ignored)
    }
}

/// Configuration for a single column's mock data generation.
#[derive(Debug, Clone, PartialEq)]
pub struct MockColumnConfig {
    pub column_name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub is_nullable: bool,
    pub generator: MockGeneratorType,
}

/// Heuristically infer the best mock generator for a column based on name, type, and constraints.
pub fn infer_mock_generator(
    col_name: &str,
    data_type: &str,
    is_pk: bool,
    is_auto_inc: bool,
) -> MockGeneratorType {
    let lower_name = col_name.to_ascii_lowercase();
    let lower_type = data_type.to_ascii_lowercase();

    // 1. Primary keys
    if is_pk {
        if is_auto_inc
            || lower_type.contains("int")
            || lower_type.contains("serial")
            || lower_name == "id"
            || lower_name.ends_with("_id")
        {
            return MockGeneratorType::AutoIncrement { start: 1, step: 1 };
        }
        if lower_type.contains("uuid") || lower_type.contains("char(36)") {
            return MockGeneratorType::UuidV4;
        }
    }

    // 2. Column Name Patterns
    if lower_name == "id" || (lower_name.ends_with("_id") && lower_type.contains("int")) {
        return MockGeneratorType::AutoIncrement { start: 1, step: 1 };
    }

    if lower_name.contains("uuid") || lower_name.contains("guid") {
        return MockGeneratorType::UuidV4;
    }

    if lower_name == "email" || lower_name.ends_with("_email") || lower_name.contains("mail") {
        return MockGeneratorType::Email;
    }

    if lower_name.contains("phone") || lower_name.contains("mobile") || lower_name.contains("tel") {
        return MockGeneratorType::PhoneNumber;
    }

    if lower_name == "name"
        || lower_name == "username"
        || lower_name == "full_name"
        || lower_name == "fullname"
        || lower_name == "nickname"
        || lower_name == "author"
        || lower_name == "user_name"
    {
        return MockGeneratorType::PersonName;
    }

    if lower_name == "status" || lower_name == "state" {
        return MockGeneratorType::EnumChoices {
            options: vec!["active".into(), "pending".into(), "inactive".into()],
        };
    }

    if lower_name == "type" || lower_name == "category" || lower_name == "role" {
        return MockGeneratorType::EnumChoices {
            options: vec!["standard".into(), "premium".into(), "admin".into()],
        };
    }

    if lower_name == "gender" || lower_name == "sex" {
        return MockGeneratorType::EnumChoices {
            options: vec!["male".into(), "female".into(), "other".into()],
        };
    }

    if lower_name == "age" {
        return MockGeneratorType::RandomInt { min: 18, max: 75 };
    }

    if lower_name.contains("price")
        || lower_name.contains("amount")
        || lower_name.contains("salary")
        || lower_name.contains("cost")
        || lower_name.contains("balance")
        || lower_name.contains("money")
        || lower_name.contains("total")
    {
        return MockGeneratorType::RandomFloat {
            min: 9.99,
            max: 999.99,
            decimals: 2,
        };
    }

    if lower_name.starts_with("is_")
        || lower_name.starts_with("has_")
        || lower_name == "enabled"
        || lower_name == "active"
        || lower_name == "verified"
    {
        return MockGeneratorType::Boolean { true_ratio: 0.8 };
    }

    if lower_name.contains("created")
        || lower_name.contains("updated")
        || lower_name.contains("timestamp")
    {
        return MockGeneratorType::DateTime { past_days: 60 };
    }

    if lower_name.contains("birth") || lower_name.contains("date") {
        return MockGeneratorType::Date {
            past_days: 365 * 20,
        };
    }

    if lower_name.contains("desc")
        || lower_name.contains("content")
        || lower_name.contains("remark")
        || lower_name.contains("comment")
        || lower_name.contains("bio")
        || lower_name.contains("summary")
        || lower_name.contains("note")
    {
        return MockGeneratorType::LoremIpsum { words: 8 };
    }

    if lower_name.contains("count")
        || lower_name.contains("score")
        || lower_name.contains("quantity")
        || lower_name.contains("rank")
        || lower_name.contains("views")
        || lower_name.contains("order_num")
    {
        return MockGeneratorType::RandomInt { min: 1, max: 500 };
    }

    // 3. Fallback by SQL Data Types
    if lower_type.contains("bool") {
        return MockGeneratorType::Boolean { true_ratio: 0.5 };
    }
    if lower_type.contains("int") || lower_type.contains("serial") {
        return MockGeneratorType::RandomInt { min: 1, max: 10000 };
    }
    if lower_type.contains("float")
        || lower_type.contains("double")
        || lower_type.contains("real")
        || lower_type.contains("decimal")
        || lower_type.contains("numeric")
    {
        return MockGeneratorType::RandomFloat {
            min: 1.0,
            max: 1000.0,
            decimals: 2,
        };
    }
    if lower_type.contains("timestamp") || lower_type.contains("datetime") {
        return MockGeneratorType::DateTime { past_days: 90 };
    }
    if lower_type.contains("date") {
        return MockGeneratorType::Date { past_days: 365 };
    }
    if lower_type.contains("uuid") {
        return MockGeneratorType::UuidV4;
    }
    if lower_type.contains("char") || lower_type.contains("text") {
        return MockGeneratorType::LoremIpsum { words: 4 };
    }

    MockGeneratorType::FixedValue {
        value: "mock".to_string(),
    }
}

/// Convert table columns into initial mock column configs.
pub fn initialize_column_configs(columns: &[ColumnInfo]) -> Vec<MockColumnConfig> {
    columns
        .iter()
        .map(|c| {
            let strategy =
                infer_mock_generator(&c.name, &c.data_type, c.is_primary_key, c.is_auto_increment);
            MockColumnConfig {
                column_name: c.name.clone(),
                data_type: c.data_type.clone(),
                is_pk: c.is_primary_key,
                is_nullable: c.is_nullable,
                generator: strategy,
            }
        })
        .collect()
}

/// Lightweight, deterministic pseudo-random number generator (SplitMix64).
#[derive(Debug, Clone)]
pub struct FastRng {
    state: u64,
}

impl FastRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    pub fn gen_range_i64(&mut self, min: i64, max: i64) -> i64 {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u64;
        min + (self.next_u64() % range) as i64
    }

    pub fn gen_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    pub fn gen_range_f64(&mut self, min: f64, max: f64) -> f64 {
        if min >= max {
            return min;
        }
        min + self.gen_f64() * (max - min)
    }

    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            let idx = (self.next_u64() as usize) % slice.len();
            Some(&slice[idx])
        }
    }
}

const FIRST_NAMES: &[&str] = &[
    "James",
    "Mary",
    "Robert",
    "Patricia",
    "John",
    "Jennifer",
    "Michael",
    "Linda",
    "David",
    "Elizabeth",
    "William",
    "Barbara",
    "Richard",
    "Susan",
    "Joseph",
    "Jessica",
    "Thomas",
    "Sarah",
    "Charles",
    "Karen",
    "Christopher",
    "Nancy",
    "Daniel",
    "Lisa",
    "Matthew",
    "Betty",
    "Anthony",
    "Margaret",
    "Mark",
    "Sandra",
    "Donald",
    "Ashley",
    "Steven",
    "Kimberly",
    "Paul",
    "Emily",
    "Andrew",
    "Donna",
    "Joshua",
    "Michelle",
    "Lucas",
    "Emma",
    "Liam",
    "Olivia",
    "Noah",
    "Ava",
    "Oliver",
    "Sophia",
    "Elijah",
    "Isabella",
];

const LAST_NAMES: &[&str] = &[
    "Smith",
    "Johnson",
    "Williams",
    "Brown",
    "Jones",
    "Garcia",
    "Miller",
    "Davis",
    "Rodriguez",
    "Martinez",
    "Hernandez",
    "Lopez",
    "Gonzalez",
    "Wilson",
    "Anderson",
    "Thomas",
    "Taylor",
    "Moore",
    "Jackson",
    "Martin",
    "Lee",
    "Perez",
    "Thompson",
    "White",
    "Harris",
    "Sanchez",
    "Clark",
    "Ramirez",
    "Lewis",
    "Robinson",
    "Walker",
    "Young",
    "Allen",
    "King",
    "Wright",
    "Scott",
    "Torres",
    "Nguyen",
    "Hill",
    "Flores",
    "Green",
    "Adams",
    "Nelson",
    "Baker",
    "Hall",
    "Rivera",
    "Campbell",
    "Mitchell",
    "Carter",
    "Roberts",
];

const EMAIL_DOMAINS: &[&str] = &[
    "gmail.com",
    "outlook.com",
    "yahoo.com",
    "icloud.com",
    "proton.me",
    "company.io",
    "tech.org",
    "example.com",
];

const LOREM_WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
    "database",
    "query",
    "index",
    "performance",
    "rust",
    "crab",
    "engine",
    "scale",
    "record",
    "transaction",
    "cluster",
    "service",
];

/// Formatted mock value ready for SQL interpolation or UI preview.
#[derive(Debug, Clone, PartialEq)]
pub enum MockSqlValue {
    Null,
    Numeric(String),
    Boolean(bool),
    Text(String),
}

impl MockSqlValue {
    pub fn to_sql_literal(&self, dialect: DatabaseFamily) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Numeric(n) => n.clone(),
            Self::Boolean(b) => match dialect {
                DatabaseFamily::Postgres => {
                    if *b {
                        "TRUE".to_string()
                    } else {
                        "FALSE".to_string()
                    }
                }
                DatabaseFamily::MySql | DatabaseFamily::Sqlite => {
                    if *b {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    }
                }
            },
            Self::Text(s) => {
                let escaped = s.replace('\'', "''");
                format!("'{}'", escaped)
            }
        }
    }

    pub fn to_display_string(&self) -> String {
        match self {
            Self::Null => "NULL".to_string(),
            Self::Numeric(n) => n.clone(),
            Self::Boolean(b) => b.to_string(),
            Self::Text(s) => s.clone(),
        }
    }
}

/// Generates a single value for a given column config, row index, and random generator.
pub fn generate_column_value(
    config: &MockColumnConfig,
    row_idx: usize,
    rng: &mut FastRng,
) -> MockSqlValue {
    match &config.generator {
        MockGeneratorType::AutoIncrement { start, step } => {
            let val = start + (row_idx as i64) * step;
            MockSqlValue::Numeric(val.to_string())
        }
        MockGeneratorType::RandomInt { min, max } => {
            let val = rng.gen_range_i64(*min, *max);
            MockSqlValue::Numeric(val.to_string())
        }
        MockGeneratorType::RandomFloat { min, max, decimals } => {
            let val = rng.gen_range_f64(*min, *max);
            MockSqlValue::Numeric(format!("{:.1$}", val, *decimals as usize))
        }
        MockGeneratorType::PersonName => {
            let first = rng.choose(FIRST_NAMES).copied().unwrap_or("John");
            let last = rng.choose(LAST_NAMES).copied().unwrap_or("Doe");
            MockSqlValue::Text(format!("{} {}", first, last))
        }
        MockGeneratorType::Email => {
            let first = rng
                .choose(FIRST_NAMES)
                .copied()
                .unwrap_or("john")
                .to_ascii_lowercase();
            let last = rng
                .choose(LAST_NAMES)
                .copied()
                .unwrap_or("doe")
                .to_ascii_lowercase();
            let num = rng.gen_range_i64(10, 999);
            let domain = rng.choose(EMAIL_DOMAINS).copied().unwrap_or("example.com");
            MockSqlValue::Text(format!("{}.{}{}@{}", first, last, num, domain))
        }
        MockGeneratorType::PhoneNumber => {
            let p1 = rng.gen_range_i64(130, 199);
            let p2 = rng.gen_range_i64(1000, 9999);
            let p3 = rng.gen_range_i64(1000, 9999);
            MockSqlValue::Text(format!("{}-{}-{}", p1, p2, p3))
        }
        MockGeneratorType::UuidV4 => {
            let u = uuid::Uuid::new_v4();
            MockSqlValue::Text(u.to_string())
        }
        MockGeneratorType::DateTime { past_days } => {
            let now = chrono::Utc::now().naive_utc();
            let seconds_in_past = rng.gen_range_i64(0, (*past_days as i64).max(1) * 86400);
            let dt = now - chrono::Duration::seconds(seconds_in_past);
            MockSqlValue::Text(dt.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        MockGeneratorType::Date { past_days } => {
            let now = chrono::Utc::now().naive_utc().date();
            let days_in_past = rng.gen_range_i64(0, (*past_days as i64).max(1));
            let d = now - chrono::Duration::days(days_in_past);
            MockSqlValue::Text(d.format("%Y-%m-%d").to_string())
        }
        MockGeneratorType::Boolean { true_ratio } => {
            let b = rng.gen_f64() < *true_ratio;
            MockSqlValue::Boolean(b)
        }
        MockGeneratorType::EnumChoices { options } => {
            if options.is_empty() {
                MockSqlValue::Text("default".to_string())
            } else {
                let chosen = rng
                    .choose(options)
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                MockSqlValue::Text(chosen)
            }
        }
        MockGeneratorType::LoremIpsum { words } => {
            let count = (*words).max(1) as usize;
            let mut result = Vec::with_capacity(count);
            for _ in 0..count {
                result.push(rng.choose(LOREM_WORDS).copied().unwrap_or("lorem"));
            }
            let mut sentence = result.join(" ");
            if let Some(first_char) = sentence.chars().next() {
                sentence.replace_range(
                    ..first_char.len_utf8(),
                    &first_char.to_uppercase().to_string(),
                );
            }
            MockSqlValue::Text(sentence)
        }
        MockGeneratorType::FixedValue { value } => MockSqlValue::Text(value.clone()),
        MockGeneratorType::Null => MockSqlValue::Null,
        MockGeneratorType::Ignored => MockSqlValue::Null,
    }
}

/// Escape identifier depending on database family.
pub fn quote_identifier(name: &str, dialect: DatabaseFamily) -> String {
    match dialect {
        DatabaseFamily::MySql => format!("`{}`", name.replace('`', "``")),
        DatabaseFamily::Postgres | DatabaseFamily::Sqlite => {
            format!("\"{}\"", name.replace('"', "\"\""))
        }
    }
}

/// Generate multi-row SQL INSERT statements for a batch of rows.
pub fn generate_batch_insert_sql(
    dialect: DatabaseFamily,
    table_name: &str,
    columns: &[MockColumnConfig],
    batch_rows: &[Vec<MockSqlValue>],
) -> String {
    let active_indices: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, col)| !col.generator.is_ignored())
        .map(|(idx, _)| idx)
        .collect();

    if active_indices.is_empty() || batch_rows.is_empty() {
        return String::new();
    }

    let quoted_table = quote_identifier(table_name, dialect);
    let column_names_sql: Vec<String> = active_indices
        .iter()
        .map(|&idx| quote_identifier(&columns[idx].column_name, dialect))
        .collect();

    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES\n",
        quoted_table,
        column_names_sql.join(", ")
    );

    let row_strings: Vec<String> = batch_rows
        .iter()
        .map(|row| {
            let values_sql: Vec<String> = active_indices
                .iter()
                .map(|&idx| {
                    if let Some(val) = row.get(idx) {
                        val.to_sql_literal(dialect)
                    } else {
                        "NULL".to_string()
                    }
                })
                .collect();
            format!("  ({})", values_sql.join(", "))
        })
        .collect();

    sql.push_str(&row_strings.join(",\n"));
    sql.push(';');
    sql
}

/// Generates preview rows (column headers and formatted display strings).
pub fn generate_mock_preview(
    columns: &[MockColumnConfig],
    count: usize,
) -> (Vec<String>, Vec<Vec<String>>) {
    let mut rng = FastRng::new(0x1337_CAFE_BABE);
    let active_cols: Vec<&MockColumnConfig> = columns
        .iter()
        .filter(|c| !c.generator.is_ignored())
        .collect();

    let headers: Vec<String> = active_cols.iter().map(|c| c.column_name.clone()).collect();
    let mut rows = Vec::with_capacity(count);

    for row_idx in 0..count {
        let mut row_vals = Vec::with_capacity(active_cols.len());
        for col in &active_cols {
            let val = generate_column_value(col, row_idx, &mut rng);
            row_vals.push(val.to_display_string());
        }
        rows.push(row_vals);
    }

    (headers, rows)
}

/// Progress callback information during execution.
#[derive(Debug, Clone, Copy)]
pub struct MockProgress {
    pub inserted: usize,
    pub total: usize,
    pub percent: f32,
}

/// Result summary after mock seeding finishes.
#[derive(Debug, Clone)]
pub struct MockResult {
    pub total_inserted: usize,
    pub elapsed_ms: u128,
}

/// Execute mock data generation and seeding in chunked transactions.
pub async fn execute_mock_seeding<F>(
    handle: &ActiveConnection,
    table_name: &str,
    columns: &[MockColumnConfig],
    total_rows: usize,
    batch_size: usize,
    mut on_progress: F,
) -> DbResult<MockResult>
where
    F: FnMut(MockProgress),
{
    if total_rows == 0 {
        return Ok(MockResult {
            total_inserted: 0,
            elapsed_ms: 0,
        });
    }

    let start_time = Instant::now();
    let dialect = handle.config.db_type.family();
    let batch_size = batch_size.clamp(10, 2000);
    let mut rng = FastRng::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xCAFE_BABE),
    );

    // Transaction begin
    let begin_sql = match dialect {
        DatabaseFamily::Sqlite => "BEGIN TRANSACTION;",
        DatabaseFamily::Postgres => "BEGIN;",
        DatabaseFamily::MySql => "START TRANSACTION;",
    };
    handle
        .execute_batch(begin_sql)
        .await
        .map_err(|e| DbError::query(e.to_string()))?;

    let mut inserted_so_far = 0;
    while inserted_so_far < total_rows {
        let current_batch_size = (total_rows - inserted_so_far).min(batch_size);
        let mut batch_rows = Vec::with_capacity(current_batch_size);

        for i in 0..current_batch_size {
            let row_idx = inserted_so_far + i;
            let mut row = Vec::with_capacity(columns.len());
            for col in columns {
                row.push(generate_column_value(col, row_idx, &mut rng));
            }
            batch_rows.push(row);
        }

        let sql = generate_batch_insert_sql(dialect, table_name, columns, &batch_rows);
        if !sql.is_empty()
            && let Err(e) = handle.execute_batch(&sql).await
        {
            // Rollback on failure
            let _ = handle.execute_batch("ROLLBACK;").await;
            return Err(DbError::query(format!(
                "Failed inserting batch ({}/{}): {}",
                inserted_so_far, total_rows, e
            )));
        }

        inserted_so_far += current_batch_size;
        let percent = (inserted_so_far as f32 / total_rows as f32) * 100.0;
        on_progress(MockProgress {
            inserted: inserted_so_far,
            total: total_rows,
            percent,
        });
    }

    // Commit transaction
    handle
        .execute_batch("COMMIT;")
        .await
        .map_err(|e| DbError::query(e.to_string()))?;

    let elapsed_ms = start_time.elapsed().as_millis();
    Ok(MockResult {
        total_inserted: inserted_so_far,
        elapsed_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_generator_inference() {
        assert_eq!(
            infer_mock_generator("id", "INTEGER", true, true),
            MockGeneratorType::AutoIncrement { start: 1, step: 1 }
        );
        assert_eq!(
            infer_mock_generator("user_id", "INT", false, false),
            MockGeneratorType::AutoIncrement { start: 1, step: 1 }
        );
        assert_eq!(
            infer_mock_generator("name", "VARCHAR(100)", false, false),
            MockGeneratorType::PersonName
        );
        assert_eq!(
            infer_mock_generator("email", "VARCHAR(255)", false, false),
            MockGeneratorType::Email
        );
        assert_eq!(
            infer_mock_generator("phone", "VARCHAR(30)", false, false),
            MockGeneratorType::PhoneNumber
        );
        assert_eq!(
            infer_mock_generator("status", "VARCHAR(20)", false, false),
            MockGeneratorType::EnumChoices {
                options: vec!["active".into(), "pending".into(), "inactive".into()]
            }
        );
        assert_eq!(
            infer_mock_generator("price", "DECIMAL(10,2)", false, false),
            MockGeneratorType::RandomFloat {
                min: 9.99,
                max: 999.99,
                decimals: 2
            }
        );
        assert_eq!(
            infer_mock_generator("is_admin", "BOOLEAN", false, false),
            MockGeneratorType::Boolean { true_ratio: 0.8 }
        );
        assert_eq!(
            infer_mock_generator("created_at", "TIMESTAMP", false, false),
            MockGeneratorType::DateTime { past_days: 60 }
        );
    }

    #[test]
    fn test_fast_rng_distribution_and_reproducibility() {
        let mut rng1 = FastRng::new(42);
        let mut rng2 = FastRng::new(42);

        for _ in 0..50 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }

        let mut rng = FastRng::new(12345);
        for _ in 0..100 {
            let v = rng.gen_range_i64(10, 20);
            assert!((10..=20).contains(&v));
        }
    }

    #[test]
    fn test_generate_mock_preview() {
        let cols = vec![
            MockColumnConfig {
                column_name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                is_pk: true,
                is_nullable: false,
                generator: MockGeneratorType::AutoIncrement { start: 1, step: 1 },
            },
            MockColumnConfig {
                column_name: "name".to_string(),
                data_type: "TEXT".to_string(),
                is_pk: false,
                is_nullable: false,
                generator: MockGeneratorType::PersonName,
            },
            MockColumnConfig {
                column_name: "hidden".to_string(),
                data_type: "TEXT".to_string(),
                is_pk: false,
                is_nullable: true,
                generator: MockGeneratorType::Ignored,
            },
        ];

        let (headers, rows) = generate_mock_preview(&cols, 5);
        assert_eq!(headers, vec!["id", "name"]);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0][0], "1");
        assert_eq!(rows[1][0], "2");
        assert!(!rows[0][1].is_empty());
    }

    #[test]
    fn test_batch_sql_generation_dialects() {
        let cols = vec![
            MockColumnConfig {
                column_name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                is_pk: true,
                is_nullable: false,
                generator: MockGeneratorType::AutoIncrement { start: 1, step: 1 },
            },
            MockColumnConfig {
                column_name: "is_active".to_string(),
                data_type: "BOOLEAN".to_string(),
                is_pk: false,
                is_nullable: false,
                generator: MockGeneratorType::Boolean { true_ratio: 1.0 },
            },
            MockColumnConfig {
                column_name: "name".to_string(),
                data_type: "TEXT".to_string(),
                is_pk: false,
                is_nullable: false,
                generator: MockGeneratorType::FixedValue {
                    value: "O'Reilly".to_string(),
                },
            },
        ];

        let rows = vec![
            vec![
                MockSqlValue::Numeric("1".to_string()),
                MockSqlValue::Boolean(true),
                MockSqlValue::Text("O'Reilly".to_string()),
            ],
            vec![
                MockSqlValue::Numeric("2".to_string()),
                MockSqlValue::Boolean(false),
                MockSqlValue::Text("Alice".to_string()),
            ],
        ];

        // PostgreSQL dialect
        let pg_sql = generate_batch_insert_sql(DatabaseFamily::Postgres, "users", &cols, &rows);
        assert!(
            pg_sql.starts_with("INSERT INTO \"users\" (\"id\", \"is_active\", \"name\") VALUES")
        );
        assert!(pg_sql.contains("(1, TRUE, 'O''Reilly')"));
        assert!(pg_sql.contains("(2, FALSE, 'Alice')"));

        // MySQL dialect
        let mysql_sql = generate_batch_insert_sql(DatabaseFamily::MySql, "users", &cols, &rows);
        assert!(mysql_sql.starts_with("INSERT INTO `users` (`id`, `is_active`, `name`) VALUES"));
        assert!(mysql_sql.contains("(1, 1, 'O''Reilly')"));

        // SQLite dialect
        let sqlite_sql = generate_batch_insert_sql(DatabaseFamily::Sqlite, "users", &cols, &rows);
        assert!(
            sqlite_sql
                .starts_with("INSERT INTO \"users\" (\"id\", \"is_active\", \"name\") VALUES")
        );
        assert!(sqlite_sql.contains("(1, 1, 'O''Reilly')"));
    }

    #[test]
    fn test_mock_values_types_and_edge_cases() {
        let mut rng = FastRng::new(999);

        // Auto increment with start 100, step 5
        let auto_cfg = MockColumnConfig {
            column_name: "seq".to_string(),
            data_type: "INT".to_string(),
            is_pk: true,
            is_nullable: false,
            generator: MockGeneratorType::AutoIncrement {
                start: 100,
                step: 5,
            },
        };
        let v0 = generate_column_value(&auto_cfg, 0, &mut rng);
        let v1 = generate_column_value(&auto_cfg, 1, &mut rng);
        let v2 = generate_column_value(&auto_cfg, 2, &mut rng);
        assert_eq!(v0.to_display_string(), "100");
        assert_eq!(v1.to_display_string(), "105");
        assert_eq!(v2.to_display_string(), "110");

        // UUID v4
        let uuid_cfg = MockColumnConfig {
            column_name: "guid".to_string(),
            data_type: "UUID".to_string(),
            is_pk: false,
            is_nullable: false,
            generator: MockGeneratorType::UuidV4,
        };
        let u_val = generate_column_value(&uuid_cfg, 0, &mut rng);
        let u_str = u_val.to_display_string();
        assert!(uuid::Uuid::parse_str(&u_str).is_ok());

        // Enum choices
        let enum_cfg = MockColumnConfig {
            column_name: "role".to_string(),
            data_type: "VARCHAR".to_string(),
            is_pk: false,
            is_nullable: false,
            generator: MockGeneratorType::EnumChoices {
                options: vec!["admin".into(), "user".into()],
            },
        };
        let role = generate_column_value(&enum_cfg, 0, &mut rng).to_display_string();
        assert!(role == "admin" || role == "user");

        // Empty rows returns empty SQL
        let empty_sql =
            generate_batch_insert_sql(DatabaseFamily::Sqlite, "users", &[auto_cfg], &[]);
        assert!(empty_sql.is_empty());
    }
}

//! CSV and data file sniffer: automatic encoding detection, delimiter inference,
//! header recognition, and sample preview generation.

use crate::db::error::{DbError, DbResult};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Supported file character encodings for data import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FileEncoding {
    #[default]
    Utf8,
    Gbk,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl FileEncoding {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Gbk => "GBK / GB18030 (Chinese)",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1252 => "Windows-1252 (Latin-1)",
        }
    }

    pub fn all() -> &'static [FileEncoding] {
        &[
            Self::Utf8,
            Self::Gbk,
            Self::Utf16Le,
            Self::Utf16Be,
            Self::Windows1252,
        ]
    }
}

/// Supported CSV field delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CsvDelimiter {
    #[default]
    Comma,
    Tab,
    Semicolon,
    Pipe,
}

impl CsvDelimiter {
    pub fn as_byte(&self) -> u8 {
        match self {
            Self::Comma => b',',
            Self::Tab => b'\t',
            Self::Semicolon => b';',
            Self::Pipe => b'|',
        }
    }

    pub fn as_char(&self) -> char {
        match self {
            Self::Comma => ',',
            Self::Tab => '\t',
            Self::Semicolon => ';',
            Self::Pipe => '|',
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Comma => "Comma (,)",
            Self::Tab => "Tab (\\t)",
            Self::Semicolon => "Semicolon (;)",
            Self::Pipe => "Pipe (|)",
        }
    }

    pub fn all() -> &'static [CsvDelimiter] {
        &[Self::Comma, Self::Tab, Self::Semicolon, Self::Pipe]
    }
}

/// Type of imported data file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImportFormat {
    #[default]
    Csv,
    Tsv,
    Sql,
}

impl ImportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Csv => "CSV (Comma Separated)",
            Self::Tsv => "TSV (Tab Separated)",
            Self::Sql => "SQL Script Batch",
        }
    }
}

/// Preview inspection result for a CSV or TSV file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPreviewData {
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub delimiter: CsvDelimiter,
    pub encoding: FileEncoding,
    pub has_headers: bool,
    pub file_size_bytes: u64,
    pub estimated_row_count: usize,
}

/// Preview inspection result for a SQL script file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlPreviewData {
    pub statement_count: usize,
    pub sample_statements: Vec<String>,
    pub file_size_bytes: u64,
}

pub struct CsvSniffer;

impl CsvSniffer {
    /// Detects character encoding by analyzing BOM signatures and byte distributions.
    pub fn detect_encoding(sample: &[u8]) -> FileEncoding {
        if sample.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return FileEncoding::Utf8;
        }
        if sample.starts_with(&[0xFF, 0xFE]) {
            return FileEncoding::Utf16Le;
        }
        if sample.starts_with(&[0xFE, 0xFF]) {
            return FileEncoding::Utf16Be;
        }

        // Try standard UTF-8 validation
        if std::str::from_utf8(sample).is_ok() {
            return FileEncoding::Utf8;
        }

        // Check for GBK / GB18030 characteristics (common in Chinese environments)
        let (decoded, _encoding_used, had_errors) = encoding_rs::GB18030.decode(sample);
        if !had_errors && !decoded.is_empty() {
            // Check if there are multi-byte sequences that look like Chinese characters
            let mut gbk_like = false;
            let mut i = 0;
            while i + 1 < sample.len() {
                let b1 = sample[i];
                let b2 = sample[i + 1];
                if (0x81..=0xFE).contains(&b1)
                    && ((0x40..=0x7E).contains(&b2) || (0x80..=0xFE).contains(&b2))
                {
                    gbk_like = true;
                    break;
                }
                i += 1;
            }
            if gbk_like {
                return FileEncoding::Gbk;
            }
        }

        FileEncoding::Utf8
    }

    /// Decodes raw bytes to a String using the specified FileEncoding.
    pub fn decode_bytes(bytes: &[u8], encoding: FileEncoding) -> String {
        match encoding {
            FileEncoding::Utf8 => {
                let slice = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    &bytes[3..]
                } else {
                    bytes
                };
                String::from_utf8_lossy(slice).into_owned()
            }
            FileEncoding::Gbk => {
                let (cow, _, _) = encoding_rs::GB18030.decode(bytes);
                cow.into_owned()
            }
            FileEncoding::Utf16Le => {
                let slice = if bytes.starts_with(&[0xFF, 0xFE]) {
                    &bytes[2..]
                } else {
                    bytes
                };
                let (cow, _, _) = encoding_rs::UTF_16LE.decode(slice);
                cow.into_owned()
            }
            FileEncoding::Utf16Be => {
                let slice = if bytes.starts_with(&[0xFE, 0xFF]) {
                    &bytes[2..]
                } else {
                    bytes
                };
                let (cow, _, _) = encoding_rs::UTF_16BE.decode(slice);
                cow.into_owned()
            }
            FileEncoding::Windows1252 => {
                let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
                cow.into_owned()
            }
        }
    }

    /// Infers field delimiter by checking consistency across multiple lines.
    pub fn infer_delimiter(sample_text: &str) -> CsvDelimiter {
        let lines: Vec<&str> = sample_text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .take(15)
            .collect();

        if lines.is_empty() {
            return CsvDelimiter::Comma;
        }

        let candidates = [
            CsvDelimiter::Comma,
            CsvDelimiter::Tab,
            CsvDelimiter::Semicolon,
            CsvDelimiter::Pipe,
        ];

        let mut best_delim = CsvDelimiter::Comma;
        let mut best_score: f64 = -1.0;

        for delim in candidates {
            let ch = delim.as_char();
            let counts: Vec<usize> = lines.iter().map(|line| line.matches(ch).count()).collect();
            let total: usize = counts.iter().sum();
            if total == 0 {
                continue;
            }

            let avg = total as f64 / counts.len() as f64;
            if avg < 1.0 {
                continue;
            }

            // Calculate standard deviation / variance across lines (lower variance = more consistent column count)
            let var = counts
                .iter()
                .map(|&c| {
                    let diff = c as f64 - avg;
                    diff * diff
                })
                .sum::<f64>()
                / counts.len() as f64;

            // Score favors higher average occurrences with low variance
            let score = avg / (1.0 + var.sqrt());
            if score > best_score {
                best_score = score;
                best_delim = delim;
            }
        }

        best_delim
    }

    /// Determines whether the first row appears to be column headers.
    pub fn detect_header(records: &[Vec<String>]) -> bool {
        if records.len() < 2 {
            return true;
        }

        let first_row = &records[0];
        let remaining_rows = &records[1..];

        // If first row has any empty column name, might not be header
        if first_row.iter().any(|val| val.trim().is_empty()) {
            return false;
        }

        // If all cells in first row are numbers, it is data rather than headers
        let first_all_numbers = first_row.iter().all(|c| c.trim().parse::<f64>().is_ok());
        if first_all_numbers {
            return false;
        }

        // Check if subsequent rows have numeric values where first row has letters
        let mut type_mismatches = 0;
        let col_count = first_row.len();
        for col_idx in 0..col_count {
            let first_val = first_row[col_idx].trim();
            let first_is_numeric = first_val.parse::<f64>().is_ok();

            let subsequent_numeric = remaining_rows
                .iter()
                .filter_map(|r| r.get(col_idx))
                .filter(|c| !c.trim().is_empty())
                .all(|c| c.trim().parse::<f64>().is_ok());

            if !first_is_numeric && subsequent_numeric {
                type_mismatches += 1;
            }
        }

        if type_mismatches > 0 {
            return true;
        }

        // Default heuristic: first row contains alphabetic names
        first_row
            .iter()
            .all(|c| c.chars().any(|ch| ch.is_alphabetic()))
    }

    /// Inspects a CSV or TSV file, auto-detecting parameters and extracting preview data.
    pub fn inspect_csv_file(
        path: &Path,
        delimiter_override: Option<CsvDelimiter>,
        encoding_override: Option<FileEncoding>,
        has_header_override: Option<bool>,
    ) -> DbResult<CsvPreviewData> {
        let file = File::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        let metadata = file.metadata().map_err(|e| DbError::Io(e.to_string()))?;
        let file_size_bytes = metadata.len();

        let mut reader = BufReader::new(file);
        let mut sample_buf = vec![0u8; 64 * 1024];
        let bytes_read = reader
            .read(&mut sample_buf)
            .map_err(|e| DbError::Io(e.to_string()))?;
        sample_buf.truncate(bytes_read);

        let encoding = encoding_override.unwrap_or_else(|| Self::detect_encoding(&sample_buf));
        let sample_text = Self::decode_bytes(&sample_buf, encoding);

        let delimiter = delimiter_override.unwrap_or_else(|| {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "tsv" {
                CsvDelimiter::Tab
            } else {
                Self::infer_delimiter(&sample_text)
            }
        });

        // Parse sample rows using csv::ReaderBuilder
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter.as_byte())
            .has_headers(false)
            .flexible(true)
            .from_reader(sample_text.as_bytes());

        let mut raw_records = Vec::new();
        for result in rdr.records().take(15) {
            match result {
                Ok(rec) => {
                    raw_records.push(rec.iter().map(|s| s.to_string()).collect::<Vec<String>>());
                }
                Err(_) => break,
            }
        }

        if raw_records.is_empty() {
            return Ok(CsvPreviewData {
                headers: Vec::new(),
                sample_rows: Vec::new(),
                delimiter,
                encoding,
                has_headers: true,
                file_size_bytes,
                estimated_row_count: 0,
            });
        }

        let has_headers = has_header_override.unwrap_or_else(|| Self::detect_header(&raw_records));

        let (headers, sample_rows) = if has_headers {
            let hdr = raw_records.remove(0);
            (hdr, raw_records)
        } else {
            let col_count = raw_records[0].len();
            let hdr = (1..=col_count).map(|i| format!("column_{i}")).collect();
            (hdr, raw_records)
        };

        // Estimate total row count based on sample average row length
        let estimated_row_count = if sample_text.is_empty() || sample_rows.is_empty() {
            0
        } else {
            let avg_line_len =
                sample_text.len() / (sample_rows.len() + if has_headers { 1 } else { 0 });
            if avg_line_len > 0 {
                (file_size_bytes as usize / avg_line_len).max(sample_rows.len())
            } else {
                sample_rows.len()
            }
        };

        Ok(CsvPreviewData {
            headers,
            sample_rows: sample_rows.into_iter().take(8).collect(),
            delimiter,
            encoding,
            has_headers,
            file_size_bytes,
            estimated_row_count,
        })
    }

    /// Inspects an external SQL script file and extracts statement statistics and preview samples.
    pub fn inspect_sql_file(path: &Path) -> DbResult<SqlPreviewData> {
        let file = File::open(path).map_err(|e| DbError::Io(e.to_string()))?;
        let metadata = file.metadata().map_err(|e| DbError::Io(e.to_string()))?;
        let file_size_bytes = metadata.len();

        let reader = BufReader::new(file);
        let mut sample_statements = Vec::new();
        let mut statement_count = 0;
        let mut current_stmt = String::new();

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.starts_with("/*") {
                continue;
            }
            if !current_stmt.is_empty() {
                current_stmt.push('\n');
            }
            current_stmt.push_str(&line);

            if trimmed.ends_with(';') {
                statement_count += 1;
                if sample_statements.len() < 5 {
                    sample_statements.push(current_stmt.trim().to_string());
                }
                current_stmt.clear();
            }
        }

        if !current_stmt.trim().is_empty() {
            statement_count += 1;
            if sample_statements.len() < 5 {
                sample_statements.push(current_stmt.trim().to_string());
            }
        }

        Ok(SqlPreviewData {
            statement_count,
            sample_statements,
            file_size_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_detect_encoding() {
        let utf8_bom = [0xEF, 0xBB, 0xBF, b'a', b'b', b'c'];
        assert_eq!(CsvSniffer::detect_encoding(&utf8_bom), FileEncoding::Utf8);

        let utf16_le = [0xFF, 0xFE, b'a', 0x00, b'b', 0x00];
        assert_eq!(
            CsvSniffer::detect_encoding(&utf16_le),
            FileEncoding::Utf16Le
        );

        let utf16_be = [0xFE, 0xFF, 0x00, b'a', 0x00, b'b'];
        assert_eq!(
            CsvSniffer::detect_encoding(&utf16_be),
            FileEncoding::Utf16Be
        );

        let plain_ascii = b"id,name,age\n1,alice,30\n";
        assert_eq!(CsvSniffer::detect_encoding(plain_ascii), FileEncoding::Utf8);
    }

    #[test]
    fn test_infer_delimiter() {
        let comma_sample = "id,name,age\n1,alice,30\n2,bob,25\n3,charlie,40\n";
        assert_eq!(
            CsvSniffer::infer_delimiter(comma_sample),
            CsvDelimiter::Comma
        );

        let tab_sample = "id\tname\tage\n1\talice\t30\n2\tbob\t25\n3\tcharlie\t40\n";
        assert_eq!(CsvSniffer::infer_delimiter(tab_sample), CsvDelimiter::Tab);

        let semi_sample = "id;name;age\n1;alice;30\n2;bob;25\n3;charlie;40\n";
        assert_eq!(
            CsvSniffer::infer_delimiter(semi_sample),
            CsvDelimiter::Semicolon
        );

        let pipe_sample = "id|name|age\n1|alice|30\n2|bob|25\n3|charlie|40\n";
        assert_eq!(CsvSniffer::infer_delimiter(pipe_sample), CsvDelimiter::Pipe);
    }

    #[test]
    fn test_detect_header() {
        let with_headers = vec![
            vec![
                "id".to_string(),
                "username".to_string(),
                "score".to_string(),
            ],
            vec!["1".to_string(), "alice".to_string(), "95.5".to_string()],
            vec!["2".to_string(), "bob".to_string(), "88.0".to_string()],
        ];
        assert!(CsvSniffer::detect_header(&with_headers));

        let without_headers = vec![
            vec!["1".to_string(), "20.5".to_string(), "100".to_string()],
            vec!["2".to_string(), "30.0".to_string(), "105".to_string()],
        ];
        assert!(!CsvSniffer::detect_header(&without_headers));
    }

    #[test]
    fn test_inspect_csv_and_sql_file() {
        let temp_dir = std::env::temp_dir();

        // 1. Inspect CSV
        let csv_path = temp_dir.join("test_zqlcrab_import.csv");
        {
            let mut f = File::create(&csv_path).unwrap();
            writeln!(f, "product_id,product_name,price").unwrap();
            writeln!(f, "1,Laptop,1200.50").unwrap();
            writeln!(f, "2,Keyboard,49.99").unwrap();
            writeln!(f, "3,Monitor,299.00").unwrap();
        }

        let preview = CsvSniffer::inspect_csv_file(&csv_path, None, None, None).unwrap();
        assert_eq!(preview.delimiter, CsvDelimiter::Comma);
        assert!(preview.has_headers);
        assert_eq!(preview.headers, vec!["product_id", "product_name", "price"]);
        assert_eq!(preview.sample_rows.len(), 3);
        assert_eq!(preview.sample_rows[0], vec!["1", "Laptop", "1200.50"]);

        let _ = std::fs::remove_file(&csv_path);

        // 2. Inspect SQL
        let sql_path = temp_dir.join("test_zqlcrab_import.sql");
        {
            let mut f = File::create(&sql_path).unwrap();
            writeln!(f, "-- Demo database backup").unwrap();
            writeln!(f, "CREATE TABLE users (id INT, name TEXT);").unwrap();
            writeln!(f, "INSERT INTO users VALUES (1, 'Alice');").unwrap();
            writeln!(f, "INSERT INTO users VALUES (2, 'Bob');").unwrap();
        }

        let sql_preview = CsvSniffer::inspect_sql_file(&sql_path).unwrap();
        assert_eq!(sql_preview.statement_count, 3);
        assert_eq!(sql_preview.sample_statements.len(), 3);

        let _ = std::fs::remove_file(&sql_path);
    }
}

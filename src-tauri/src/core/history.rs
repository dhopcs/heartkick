//! Sample history backed by SQLite via sqlx.
//!
//! [`SqliteHistory`] is the default; the [`HistoryStore`] trait exists so
//! tests or future backends can substitute without touching callers.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::str::FromStr;

use crate::bluetooth::HrSample;

#[async_trait]
pub trait HistoryStore: Send + Sync {
    /// Append a new sample.
    async fn push(&self, sample: HrSample);
    /// Return up to `limit` most recent samples, oldest first.
    async fn recent(&self, limit: usize) -> Vec<HrSample>;
}

/// SQLite-backed persistent store.
pub struct SqliteHistory {
    pool: SqlitePool,
    /// Counts pushes so pruning only runs every 100 samples.
    push_count: AtomicU64,
}

impl SqliteHistory {
    /// Open (or create) the SQLite database at `path` and run migrations.
    pub async fn open(path: std::path::PathBuf) -> Result<Self> {
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .context("building sqlite connect options")?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            // 64 pages × 4 KB = 256 KB page cache (down from 2 MB default).
            .pragma("cache_size", "-64")
            // Keep temp tables in memory rather than on disk.
            .pragma("temp_store", "memory")
            // Disable memory-mapped I/O; let the OS page cache do its job.
            .pragma("mmap_size", "0");

        let pool = SqlitePoolOptions::new()
            // Single connection is sufficient: WAL allows concurrent readers
            // and we only ever append one sample at a time.
            .max_connections(1)
            .connect_with(opts)
            .await
            .context("opening sqlite pool")?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS hr_samples (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                bpm       INTEGER NOT NULL,
                rr        TEXT NOT NULL,
                ts        TEXT NOT NULL
             )",
        )
        .execute(&pool)
        .await
        .context("creating hr_samples table")?;

        // Keep 30 days of data by default (trimmed on each push).
        Ok(Self {
            pool,
            push_count: AtomicU64::new(0),
        })
    }
}

#[async_trait]
impl HistoryStore for SqliteHistory {
    async fn push(&self, sample: HrSample) {
        let rr = sample
            .rr_intervals_ms
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let ts = sample.timestamp.to_rfc3339();
        let bpm = sample.bpm as i64;

        if let Err(e) = sqlx::query("INSERT INTO hr_samples (bpm, rr, ts) VALUES (?, ?, ?)")
            .bind(bpm)
            .bind(&rr)
            .bind(&ts)
            .execute(&self.pool)
            .await
        {
            tracing::warn!(error = %e, "sqlite push failed");
            return;
        }

        // Prune rows older than 30 days, but only every 100 samples.
        let n = self.push_count.fetch_add(1, Ordering::Relaxed);
        if n.is_multiple_of(100) {
            let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
            let _ = sqlx::query("DELETE FROM hr_samples WHERE ts < ?")
                .bind(&cutoff)
                .execute(&self.pool)
                .await;
        }
    }

    async fn recent(&self, limit: usize) -> Vec<HrSample> {
        let rows = match sqlx::query("SELECT bpm, rr, ts FROM hr_samples ORDER BY id DESC LIMIT ?")
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "sqlite recent query failed");
                return Vec::new();
            }
        };

        let mut out: Vec<HrSample> = rows
            .iter()
            .filter_map(|row| {
                let bpm: i64 = row.try_get("bpm").ok()?;
                let rr_str: String = row.try_get("rr").ok()?;
                let ts_str: String = row.try_get("ts").ok()?;
                let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                    .ok()?
                    .with_timezone(&Utc);
                let rr_intervals_ms: Vec<u16> = if rr_str.is_empty() {
                    Vec::new()
                } else {
                    rr_str.split(',').filter_map(|s| s.parse().ok()).collect()
                };
                Some(HrSample {
                    bpm: bpm as u16,
                    rr_intervals_ms,
                    timestamp,
                })
            })
            .collect();

        // Rows came DESC, reverse to oldest-first.
        out.reverse();
        out
    }
}

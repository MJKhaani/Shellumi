//! Micro-benchmark harness and performance verification engine.
//!
//! Provides deterministic offline benchmarks to verify:
//! - Cryptographic throughput: AES-256-GCM AEAD encryption/decryption (MB/s).
//! - Key derivation latency: PBKDF2-HMAC-SHA256 stretching latency (μs/op).
//! - Autocomplete responsiveness: query latency across large dictionaries (< 2 ms/query).
//! - Template & automation throughput: parameter substitution & safety checks (ops/sec).
//! - Sync difference engine: decrypted record comparison and conflict detection (records/sec).
//! - Network rule validation: port-forward spec parsing and localhost exposure auditing (ops/sec).

use crate::autocomplete::{AutocompleteContext, AutocompleteEngine};
use crate::startup::{prepare_startup_snippet, StartupSnippetContext};
use crate::sync::{
    diff_sync_records_with_passphrase, encrypt_entity_with_iterations, open_payload, seal_payload,
    SyncEntityType, SyncRecord, KEY_LEN,
};
use crate::tunnel::{ForwardingType, PortForwardRule};
use ring::aead::NONCE_LEN;
use ring::pbkdf2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU32;
use std::time::Instant;

/// Benchmark category classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BenchmarkCategory {
    Crypto,
    Security,
    Autocomplete,
    Template,
    Sync,
    Network,
}

impl BenchmarkCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Crypto => "Cryptographic AEAD",
            Self::Security => "Key Derivation & Hashing",
            Self::Autocomplete => "In-Terminal Autocomplete",
            Self::Template => "Snippet & Template Engine",
            Self::Sync => "Sync Difference Engine",
            Self::Network => "Port-Forwarding & Rules",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Crypto => "🔒",
            Self::Security => "🛡️",
            Self::Autocomplete => "⚡",
            Self::Template => "📜",
            Self::Sync => "🔄",
            Self::Network => "🚇",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "crypto" | "aead" | "aes" => Some(Self::Crypto),
            "security" | "kdf" | "pbkdf2" => Some(Self::Security),
            "autocomplete" | "complete" => Some(Self::Autocomplete),
            "template" | "snippet" => Some(Self::Template),
            "sync" | "diff" => Some(Self::Sync),
            "network" | "tunnel" | "port" => Some(Self::Network),
            _ => None,
        }
    }
}

/// SLA compliance status for an individual benchmark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BenchmarkStatus {
    Passed,
    Warning,
    Failed,
}

impl BenchmarkStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Passed => "✅ PASS",
            Self::Warning => "⚠️ WARN",
            Self::Failed => "❌ FAIL",
        }
    }
}

impl fmt::Display for BenchmarkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.badge())
    }
}

/// Result metrics for a single executed benchmark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkMetric {
    pub name: String,
    pub category: BenchmarkCategory,
    pub description: String,
    pub iterations: usize,
    pub duration_ms: f64,
    pub ops_per_sec: f64,
    pub throughput_mb_per_sec: Option<f64>,
    pub latency_us_per_op: f64,
    pub status: BenchmarkStatus,
    pub target_sla: String,
    pub note: String,
}

/// Aggregated report from running benchmark suites.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp_iso: String,
    pub total_duration_ms: f64,
    pub metrics: Vec<BenchmarkMetric>,
    pub passed_count: usize,
    pub warning_count: usize,
    pub failed_count: usize,
}

impl BenchmarkReport {
    pub fn is_healthy(&self) -> bool {
        self.failed_count == 0
    }

    /// Formats the benchmark report as an aligned, human-readable terminal table.
    pub fn format_terminal_table(&self) -> String {
        let mut out = String::new();
        out.push_str("========================================================================================\n");
        out.push_str("                       SHELLUMI PERFORMANCE BENCHMARK REPORT                          \n");
        out.push_str("========================================================================================\n");
        out.push_str(&format!(
            "Total Benchmarks: {} | Passed: {} | Warnings: {} | Failed: {} | Total Time: {:.2}ms\n\n",
            self.metrics.len(),
            self.passed_count,
            self.warning_count,
            self.failed_count,
            self.total_duration_ms
        ));

        out.push_str(&format!(
            "{:<4} {:<24} {:<10} {:<12} {:<16} {:<10} {}\n",
            "STAT", "BENCHMARK", "ITERS", "TIME (ms)", "OPS/SEC", "LATENCY", "SLA REQUIREMENT"
        ));
        out.push_str(&"-".repeat(96));
        out.push('\n');

        for m in &self.metrics {
            let latency_display = if m.latency_us_per_op >= 1000.0 {
                format!("{:.2} ms", m.latency_us_per_op / 1000.0)
            } else {
                format!("{:.1} µs", m.latency_us_per_op)
            };

            let ops_display = if let Some(mbps) = m.throughput_mb_per_sec {
                format!("{:.1} MB/s", mbps)
            } else if m.ops_per_sec >= 1000.0 {
                format!("{:.0} k/s", m.ops_per_sec / 1000.0)
            } else {
                format!("{:.0} /s", m.ops_per_sec)
            };

            out.push_str(&format!(
                "{:<4} {:<24} {:<10} {:<12.2} {:<16} {:<10} {}\n",
                m.status.badge(),
                m.name,
                m.iterations,
                m.duration_ms,
                ops_display,
                latency_display,
                m.target_sla
            ));
        }

        out.push_str(&"-".repeat(96));
        out.push('\n');

        if self.is_healthy() {
            out.push_str("Result: ALL PERFORMANCE BENCHMARKS SATISFIED SLA REQUIREMENTS.\n");
        } else {
            out.push_str("Result: SOME PERFORMANCE BENCHMARKS FAILED SLA REQUIREMENTS.\n");
        }
        out
    }

    /// Formats the benchmark report as GitHub Flavored Markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Shellumi Performance Benchmark Report\n\n");
        md.push_str(&format!(
            "- **Execution Timestamp:** {}\n- **Total Duration:** {:.2} ms\n- **Status:** {} ({} Passed, {} Warnings, {} Failed)\n\n",
            self.timestamp_iso,
            self.total_duration_ms,
            if self.is_healthy() { "✅ Healthy" } else { "⚠️ Degradation Detected" },
            self.passed_count,
            self.warning_count,
            self.failed_count
        ));

        md.push_str("| Status | Category | Benchmark | Iterations | Duration | Throughput / Speed | Avg Latency | SLA Target |\n");
        md.push_str("|:------:|:---------|:----------|-----------:|---------:|-------------------:|------------:|:-----------|\n");

        for m in &self.metrics {
            let throughput_col = if let Some(mbps) = m.throughput_mb_per_sec {
                format!("{:.2} MB/s", mbps)
            } else {
                format!("{:.0} ops/s", m.ops_per_sec)
            };

            let latency_col = if m.latency_us_per_op >= 1000.0 {
                format!("{:.2} ms", m.latency_us_per_op / 1000.0)
            } else {
                format!("{:.1} µs", m.latency_us_per_op)
            };

            md.push_str(&format!(
                "| {} | {} {} | `{}` | {} | {:.2} ms | {} | {} | {} |\n",
                m.status.badge(),
                m.category.icon(),
                m.category.display_name(),
                m.name,
                m.iterations,
                m.duration_ms,
                throughput_col,
                latency_col,
                m.target_sla
            ));
        }

        md.push('\n');
        md
    }

    /// Serializes report as structured JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Micro-benchmark suite runner.
pub struct BenchmarkRunner;

impl BenchmarkRunner {
    /// Runs all micro-benchmarks or a filtered category.
    pub fn run_suite(filter: Option<BenchmarkCategory>) -> BenchmarkReport {
        let overall_start = Instant::now();
        let mut metrics = Vec::new();

        if filter.is_none() || filter == Some(BenchmarkCategory::Crypto) {
            metrics.push(Self::bench_aes_gcm_throughput());
        }

        if filter.is_none() || filter == Some(BenchmarkCategory::Security) {
            metrics.push(Self::bench_pbkdf2_key_derivation());
        }

        if filter.is_none() || filter == Some(BenchmarkCategory::Autocomplete) {
            metrics.push(Self::bench_autocomplete_search());
        }

        if filter.is_none() || filter == Some(BenchmarkCategory::Template) {
            metrics.push(Self::bench_startup_template_rendering());
        }

        if filter.is_none() || filter == Some(BenchmarkCategory::Sync) {
            metrics.push(Self::bench_sync_diff_engine());
        }

        if filter.is_none() || filter == Some(BenchmarkCategory::Network) {
            metrics.push(Self::bench_port_forward_validation());
        }

        let total_duration_ms = overall_start.elapsed().as_secs_f64() * 1000.0;
        let mut passed_count = 0;
        let mut warning_count = 0;
        let mut failed_count = 0;

        for m in &metrics {
            match m.status {
                BenchmarkStatus::Passed => passed_count += 1,
                BenchmarkStatus::Warning => warning_count += 1,
                BenchmarkStatus::Failed => failed_count += 1,
            }
        }

        BenchmarkReport {
            timestamp_iso: "2026-11-05T00:00:00Z".to_string(),
            total_duration_ms,
            metrics,
            passed_count,
            warning_count,
            failed_count,
        }
    }

    /// Benchmarks AES-256-GCM AEAD encryption and decryption throughput.
    pub fn bench_aes_gcm_throughput() -> BenchmarkMetric {
        let iterations = 100;
        let chunk_size = 64 * 1024; // 64 KB per chunk
        let payload = vec![0x42u8; chunk_size];
        let key = [0x5au8; KEY_LEN];
        let nonce = [0x19u8; NONCE_LEN];
        let aad = b"shellumi-bench-aead-associated-data";

        let start = Instant::now();
        let mut total_bytes = 0usize;

        for _ in 0..iterations {
            let ciphertext =
                seal_payload(&key, &nonce, &payload, aad).expect("AES-GCM encryption failed");
            total_bytes += payload.len();

            let decrypted =
                open_payload(&key, &nonce, &ciphertext, aad).expect("AES-GCM decryption failed");
            assert_eq!(decrypted.len(), payload.len());
            total_bytes += decrypted.len();
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let ops_per_sec = (iterations * 2) as f64 / elapsed_secs.max(0.000_001);
        let throughput_mb_per_sec =
            (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / (iterations * 2) as f64;

        let status = if throughput_mb_per_sec >= 20.0 {
            BenchmarkStatus::Passed
        } else if throughput_mb_per_sec >= 10.0 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "aes_256_gcm_roundtrip".to_string(),
            category: BenchmarkCategory::Crypto,
            description: "Measures 64KB AES-256-GCM AEAD encryption & decryption throughput"
                .to_string(),
            iterations: iterations * 2,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: Some(throughput_mb_per_sec),
            latency_us_per_op: latency_us,
            status,
            target_sla: ">= 20.0 MB/s".to_string(),
            note: format!(
                "Processed {:.1} MB in {:.1}ms",
                (total_bytes as f64) / (1024.0 * 1024.0),
                duration_ms
            ),
        }
    }

    /// Benchmarks PBKDF2-HMAC-SHA256 key derivation stretching latency.
    pub fn bench_pbkdf2_key_derivation() -> BenchmarkMetric {
        let iterations = 3;
        let rounds = NonZeroU32::new(100_000).unwrap();
        let salt = [0x5au8; 32];
        let passphrase = b"secure_master_vault_passphrase_for_testing";

        let start = Instant::now();
        for _ in 0..iterations {
            let mut key = [0u8; 32];
            pbkdf2::derive(
                pbkdf2::PBKDF2_HMAC_SHA256,
                rounds,
                &salt,
                passphrase,
                &mut key,
            );
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let ops_per_sec = iterations as f64 / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / iterations as f64;

        // PBKDF2 100k rounds should take at least 5ms (adequate resistance).
        // In unoptimized debug test builds, allow up to 5000ms. In release builds, SLA is 1000ms.
        let max_latency_us = if cfg!(debug_assertions) {
            5_000_000.0
        } else {
            1_000_000.0
        };
        let status = if (5_000.0..=max_latency_us).contains(&latency_us) {
            BenchmarkStatus::Passed
        } else if latency_us > max_latency_us && latency_us < max_latency_us * 1.5 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "pbkdf2_sha256_100k".to_string(),
            category: BenchmarkCategory::Security,
            description: "Measures PBKDF2-HMAC-SHA256 stretching latency (100,000 rounds)"
                .to_string(),
            iterations,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: None,
            latency_us_per_op: latency_us,
            status,
            target_sla: "5 ms <= latency <= 1000 ms".to_string(),
            note: format!("Avg derivation: {:.2} ms per unlock", latency_us / 1000.0),
        }
    }

    /// Benchmarks autocomplete search responsiveness against command grammar.
    pub fn bench_autocomplete_search() -> BenchmarkMetric {
        let engine = AutocompleteEngine::new();

        let queries = [
            ("git checkout ", 13),
            ("docker run -d ", 14),
            ("ssh -p 2222 ", 12),
            ("systemctl restart ", 18),
            ("kubectl get pods ", 17),
            ("curl -X POST ", 13),
            ("tail -f /var/log/syslog", 23),
            ("journalctl -u nginx -f", 22),
        ];
        let iterations = 500;

        let start = Instant::now();
        let mut total_matches = 0;
        for i in 0..iterations {
            let (line, cursor) = queries[i % queries.len()];
            let ctx = AutocompleteContext::new(line, cursor);
            let matches = engine.complete(&ctx);
            total_matches += matches.len();
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let ops_per_sec = iterations as f64 / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / iterations as f64;

        let status = if latency_us <= 2000.0 {
            BenchmarkStatus::Passed
        } else if latency_us <= 5000.0 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "autocomplete_engine_query".to_string(),
            category: BenchmarkCategory::Autocomplete,
            description: "Evaluates in-terminal completion grammar matching and suggestion ranking"
                .to_string(),
            iterations,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: None,
            latency_us_per_op: latency_us,
            status,
            target_sla: "<= 2.0 ms / query".to_string(),
            note: format!(
                "Avg query latency: {:.1} µs ({} suggestions produced)",
                latency_us, total_matches
            ),
        }
    }

    /// Benchmarks startup snippet template rendering, variable substitution, and safety validation.
    pub fn bench_startup_template_rendering() -> BenchmarkMetric {
        let iterations = 1000;
        let template = "echo 'Connecting to {{host.name}} at {{host.hostname}}:{{host.port}} as {{host.user}}' && uptime";
        let mut custom = HashMap::new();
        custom.insert("env".to_string(), "production".to_string());

        let mut ctx = StartupSnippetContext::new("bench-srv", "192.168.1.100", "admin", 2222);
        ctx.custom_vars = custom;

        let start = Instant::now();
        for _ in 0..iterations {
            let rendered = prepare_startup_snippet(template, &ctx).expect("render failed");
            assert!(rendered.contains("192.168.1.100:2222"));
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let ops_per_sec = iterations as f64 / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / iterations as f64;

        let status = if ops_per_sec >= 10_000.0 {
            BenchmarkStatus::Passed
        } else if ops_per_sec >= 5_000.0 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "startup_template_resolution".to_string(),
            category: BenchmarkCategory::Template,
            description:
                "Measures snippet template parsing, variable injection & danger validation"
                    .to_string(),
            iterations,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: None,
            latency_us_per_op: latency_us,
            status,
            target_sla: ">= 10,000 ops/s".to_string(),
            note: format!(
                "{:.0} template renders/sec ({:.2} µs/op)",
                ops_per_sec, latency_us
            ),
        }
    }

    /// Benchmarks zero-knowledge sync difference evaluation and three-way action classification.
    pub fn bench_sync_diff_engine() -> BenchmarkMetric {
        let iterations = 20;
        let count_per_side = 10;
        let passphrase = "bench_sync_diff_passphrase";

        // Generate synthetic encrypted records
        let mut local_records = Vec::with_capacity(count_per_side);
        let mut remote_records = Vec::with_capacity(count_per_side);

        for i in 0..count_per_side {
            let object_id = format!("sync-bench-host-{i}");
            let envelope = encrypt_entity_with_iterations(
                passphrase,
                "host",
                &object_id,
                &format!("Host {i}"),
                100,
            )
            .expect("encrypt error");

            let mut sync_rec = SyncRecord::new(
                object_id,
                SyncEntityType::Host,
                1,
                "device-bench-1",
                1700000000,
                envelope,
            );

            local_records.push(sync_rec.clone());

            // Vary remote record version for odd items to trigger diff analysis
            if i % 2 == 1 {
                sync_rec.version = 2;
                sync_rec.updated_at = 1700000100;
            }
            remote_records.push(sync_rec);
        }

        let start = Instant::now();
        let mut total_diffs = 0;

        for _ in 0..iterations {
            let diffs = diff_sync_records_with_passphrase(
                &local_records,
                &remote_records,
                Some(passphrase),
            );
            total_diffs += diffs.len();
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let total_comparisons = iterations * count_per_side;
        let ops_per_sec = total_comparisons as f64 / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / total_comparisons as f64;

        let status = if ops_per_sec >= 100.0 {
            BenchmarkStatus::Passed
        } else if ops_per_sec >= 50.0 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "sync_diff_and_three_way_eval".to_string(),
            category: BenchmarkCategory::Sync,
            description:
                "Measures encrypted sync diffing, payload decryption, and conflict detection"
                    .to_string(),
            iterations: total_comparisons,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: None,
            latency_us_per_op: latency_us,
            status,
            target_sla: ">= 100 records/s".to_string(),
            note: format!(
                "{:.0} records/sec ({} total diff elements)",
                ops_per_sec, total_diffs
            ),
        }
    }

    /// Benchmarks port forwarding specification parsing, formatting, and security auditing.
    pub fn bench_port_forward_validation() -> BenchmarkMetric {
        let iterations = 2000;
        let specs = [
            ("8080:localhost:80", ForwardingType::Local),
            ("127.0.0.1:3000:192.168.1.10:3000", ForwardingType::Local),
            (
                "0.0.0.0:8443:db-server.internal:5432",
                ForwardingType::Local,
            ),
            ("1080", ForwardingType::Dynamic),
            ("9000:localhost:22", ForwardingType::Remote),
        ];

        let start = Instant::now();
        let mut exposure_warnings = 0;

        for i in 0..iterations {
            let (spec, fwd_type) = specs[i % specs.len()];
            if let Ok(rule) = PortForwardRule::parse(spec, fwd_type) {
                if rule.is_exposed_beyond_localhost() {
                    exposure_warnings += 1;
                }
                let (_flag, arg) = rule.openssh_argument();
                assert!(!arg.is_empty());
            }
        }

        let elapsed_secs = start.elapsed().as_secs_f64();
        let duration_ms = elapsed_secs * 1000.0;
        let ops_per_sec = iterations as f64 / elapsed_secs.max(0.000_001);
        let latency_us = (duration_ms * 1000.0) / iterations as f64;

        let status = if ops_per_sec >= 40_000.0 {
            BenchmarkStatus::Passed
        } else if ops_per_sec >= 20_000.0 {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Failed
        };

        BenchmarkMetric {
            name: "port_forward_spec_parsing".to_string(),
            category: BenchmarkCategory::Network,
            description:
                "Measures port forward rule parsing, serialization, and exposure verification"
                    .to_string(),
            iterations,
            duration_ms,
            ops_per_sec,
            throughput_mb_per_sec: None,
            latency_us_per_op: latency_us,
            status,
            target_sla: ">= 40,000 ops/s".to_string(),
            note: format!(
                "{:.0} rules/sec ({} flagged as public exposures)",
                ops_per_sec, exposure_warnings
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_benchmark_report_generation_and_sla() {
        let report = BenchmarkRunner::run_suite(Some(BenchmarkCategory::Network));
        assert!(!report.metrics.is_empty());
        assert_eq!(report.metrics[0].category, BenchmarkCategory::Network);
        assert!(report.total_duration_ms > 0.0);
        assert!(
            report.passed_count + report.warning_count + report.failed_count
                == report.metrics.len()
        );

        let table = report.format_terminal_table();
        assert!(table.contains("SHELLUMI PERFORMANCE BENCHMARK REPORT"));
        assert!(table.contains("port_forward_spec_parsing"));

        let md = report.to_markdown();
        assert!(md.contains("# Shellumi Performance Benchmark Report"));
        assert!(md.contains("Port-Forwarding & Rules"));

        let json = report.to_json().expect("json serialize");
        assert!(json.contains("port_forward_spec_parsing"));
    }

    #[test]
    fn verifies_all_benchmark_categories_execute_offline() {
        let report = BenchmarkRunner::run_suite(None);
        println!("{}", report.format_terminal_table());
        assert_eq!(report.metrics.len(), 6);
        assert!(
            report.is_healthy(),
            "expected zero failures in benchmark suite"
        );

        for m in &report.metrics {
            assert!(m.ops_per_sec > 0.0);
            assert!(m.duration_ms > 0.0);
            assert!(m.latency_us_per_op > 0.0);
            assert_ne!(
                m.status,
                BenchmarkStatus::Failed,
                "metric {} failed SLA",
                m.name
            );
        }
    }
}

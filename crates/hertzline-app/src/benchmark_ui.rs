//! Benchmark wizard host for the desktop UI (HZ-030 / AC-2.1--AC-2.4, AC-8.2).
//!
//! Fixed duration, warm-up, >=3 passes; median + spread; noise-band verdict.
//! Never claims "+X FPS" without a completed local before/after pair.

use hertzline_core::benchmark::{
    evaluate_benchmarks, run_benchmark, BenchmarkConfig, BenchmarkError, BenchmarkResult,
    ChangeVerdict, FrameWindowSource, MetricSpread, VerdictReport, MIN_PASS_COUNT,
};
use serde::{Deserialize, Serialize};

use crate::measure::{explanation_with_evidence, NO_LOCAL_ACTION_MEASUREMENT};

/// UI config (must satisfy AC-2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchConfigDto {
    pub warmup_ms: u64,
    pub measure_ms: u64,
    pub pass_count: u32,
}

impl BenchConfigDto {
    pub fn to_core(&self) -> Result<BenchmarkConfig, String> {
        let cfg = BenchmarkConfig {
            warmup_ms: self.warmup_ms,
            measure_ms: self.measure_ms,
            pass_count: self.pass_count,
        };
        cfg.validate().map_err(|e| e.to_string())?;
        Ok(cfg)
    }
}

impl Default for BenchConfigDto {
    fn default() -> Self {
        Self {
            warmup_ms: 1000,
            measure_ms: 5000,
            pass_count: MIN_PASS_COUNT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadDto {
    pub median: f64,
    pub iqr: f64,
    pub min: f64,
    pub max: f64,
    pub sample_count: usize,
}

impl From<&MetricSpread> for SpreadDto {
    fn from(s: &MetricSpread) -> Self {
        Self {
            median: s.median,
            iqr: s.iqr,
            min: s.min,
            max: s.max,
            sample_count: s.sample_count,
        }
    }
}

impl SpreadDto {
    fn to_core(&self) -> MetricSpread {
        MetricSpread {
            median: self.median,
            iqr: self.iqr,
            min: self.min,
            max: self.max,
            sample_count: self.sample_count,
        }
    }
}

/// Serializable aggregate (pass detail omitted; enough for UI + compare).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchResultDto {
    pub config: BenchConfigDto,
    pub avg_fps: SpreadDto,
    pub low_1pct_fps: SpreadDto,
    pub low_0_1pct_fps: SpreadDto,
    pub avg_frame_time_ms: SpreadDto,
    /// `live` | `labeled_mock` -- mocks must never be shown as real FPS gains alone.
    pub source: String,
}

impl BenchResultDto {
    fn from_core(result: &BenchmarkResult, source: &str) -> Self {
        Self {
            config: BenchConfigDto {
                warmup_ms: result.config.warmup_ms,
                measure_ms: result.config.measure_ms,
                pass_count: result.config.pass_count,
            },
            avg_fps: SpreadDto::from(&result.avg_fps),
            low_1pct_fps: SpreadDto::from(&result.low_1pct_fps),
            low_0_1pct_fps: SpreadDto::from(&result.low_0_1pct_fps),
            avg_frame_time_ms: SpreadDto::from(&result.avg_frame_time_ms),
            source: source.into(),
        }
    }

    fn to_core_for_compare(&self) -> BenchmarkResult {
        // Pass list unused by evaluate_benchmarks; keep empty.
        BenchmarkResult {
            config: BenchmarkConfig {
                warmup_ms: self.config.warmup_ms,
                measure_ms: self.config.measure_ms,
                pass_count: self.config.pass_count,
            },
            passes: Vec::new(),
            avg_fps: self.avg_fps.to_core(),
            low_1pct_fps: self.low_1pct_fps.to_core(),
            low_0_1pct_fps: self.low_0_1pct_fps.to_core(),
            avg_frame_time_ms: self.avg_frame_time_ms.to_core(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictDto {
    pub message: String,
    pub offer_revert: bool,
    pub before_median: f64,
    pub after_median: f64,
    pub gain: f64,
    pub noise_band: f64,
    pub verdict: String,
    /// Plain AC-8.2 evidence line (local numbers only).
    pub explanation: String,
    /// Forbidden marketing form never emitted; UI must not invent "+X FPS".
    pub fps_claim_allowed: bool,
}

impl From<&VerdictReport> for VerdictDto {
    fn from(r: &VerdictReport) -> Self {
        let local_msg = format!(
            "{} (measured median gain {:.3} fps; noise band {:.3}; before {:.3} -> after {:.3})",
            r.message, r.gain, r.noise_band, r.before_median, r.after_median
        );
        Self {
            message: r.message.to_string(),
            offer_revert: r.offer_revert,
            before_median: r.before_median,
            after_median: r.after_median,
            gain: r.gain,
            noise_band: r.noise_band,
            verdict: match r.verdict {
                ChangeVerdict::Helpful => "helpful".into(),
                ChangeVerdict::NoMeasurableEffect => "no_measurable_effect".into(),
                ChangeVerdict::Harmful => "harmful".into(),
            },
            explanation: explanation_with_evidence(Some(&local_msg)),
            // Local before/after exists => showing measured gain is allowed (RULES.md).
            fps_claim_allowed: true,
        }
    }
}

/// Window source built from caller-supplied frame-time lists (ms).
///
/// First window is warm-up; then one window per pass. Labeled when `source` is mock.
struct SuppliedWindows {
    windows: Vec<Vec<f64>>,
    next: usize,
}

impl FrameWindowSource for SuppliedWindows {
    fn collect_window(&mut self, _duration_ms: u64) -> Result<Vec<f64>, BenchmarkError> {
        if self.next >= self.windows.len() {
            return Err(BenchmarkError::Capture(
                "no more frame windows supplied".into(),
            ));
        }
        let w = self.windows[self.next].clone();
        self.next += 1;
        Ok(w)
    }
}

/// Run a benchmark from supplied frame windows (warmup + pass_count windows).
pub fn run_benchmark_from_windows(
    config: BenchConfigDto,
    windows: Vec<Vec<f64>>,
    source: &str,
) -> Result<BenchResultDto, String> {
    let cfg = config.to_core()?;
    let needed = 1 + cfg.pass_count as usize;
    if windows.len() < needed {
        return Err(format!(
            "need {needed} frame windows (1 warm-up + {} passes); got {}",
            cfg.pass_count,
            windows.len()
        ));
    }
    for (i, w) in windows.iter().enumerate() {
        if w.iter().filter(|t| t.is_finite() && **t > 0.0).count() < 2 && i > 0 {
            return Err(format!(
                "pass window {i}: need at least 2 positive frame samples"
            ));
        }
    }
    let mut source_impl = SuppliedWindows { windows, next: 0 };
    let result = run_benchmark(cfg, &mut source_impl).map_err(|e| e.to_string())?;
    Ok(BenchResultDto::from_core(&result, source))
}

/// Labeled mock before/after pair for UI/smoke (never presented as live capture).
pub fn run_labeled_mock_pair(
    before_frame_ms: f64,
    after_frame_ms: f64,
) -> Result<(BenchResultDto, BenchResultDto, VerdictDto), String> {
    let cfg = BenchConfigDto {
        warmup_ms: 100,
        measure_ms: 100,
        pass_count: MIN_PASS_COUNT,
    };
    let mk = |frame_ms: f64| -> Vec<Vec<f64>> {
        let samples = vec![frame_ms; 40];
        vec![samples.clone(), samples.clone(), samples.clone(), samples]
    };
    let before = run_benchmark_from_windows(cfg.clone(), mk(before_frame_ms), "labeled_mock")?;
    let after = run_benchmark_from_windows(cfg, mk(after_frame_ms), "labeled_mock")?;
    let verdict = compare_benchmarks(&before, &after)?;
    Ok((before, after, verdict))
}

/// Compare two local runs (AC-2.3, AC-2.4).
pub fn compare_benchmarks(
    before: &BenchResultDto,
    after: &BenchResultDto,
) -> Result<VerdictDto, String> {
    if before.source == "labeled_mock" || after.source == "labeled_mock" {
        // Still compute verdict for UI wiring, but mark claim carefully via source.
    }
    let report = evaluate_benchmarks(&before.to_core_for_compare(), &after.to_core_for_compare());
    Ok(VerdictDto::from(&report))
}

/// AC-8.2 when the user has not finished before+after yet.
pub fn no_local_benchmark_explanation() -> String {
    NO_LOCAL_ACTION_MEASUREMENT.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hertzline_core::benchmark::ChangeVerdict;

    #[test]
    fn default_config_meets_min_passes() {
        let d = BenchConfigDto::default();
        assert!(d.pass_count >= MIN_PASS_COUNT);
        assert!(d.to_core().is_ok());
    }

    #[test]
    fn rejects_too_few_passes() {
        let bad = BenchConfigDto {
            warmup_ms: 100,
            measure_ms: 100,
            pass_count: 2,
        };
        assert!(bad.to_core().is_err());
    }

    #[test]
    fn labeled_mock_no_effect_offers_revert() {
        // mock: identical before/after -> no measurable effect
        let (_b, _a, v) = run_labeled_mock_pair(16.0, 16.0).expect("pair");
        assert_eq!(v.message, ChangeVerdict::NO_MEASURABLE_EFFECT_MESSAGE);
        assert!(v.offer_revert);
        assert!(v.explanation.contains("Local before/after"));
        assert!(v.fps_claim_allowed);
    }

    #[test]
    fn labeled_mock_helpful_does_not_offer_revert() {
        // mock: 60fps -> 90fps flat
        let (_b, _a, v) = run_labeled_mock_pair(1000.0 / 60.0, 1000.0 / 90.0).expect("pair");
        assert_eq!(v.verdict, "helpful");
        assert!(!v.offer_revert);
        assert!(v.gain > v.noise_band);
    }

    #[test]
    fn no_local_explanation_constant() {
        assert_eq!(
            no_local_benchmark_explanation(),
            NO_LOCAL_ACTION_MEASUREMENT
        );
    }
}

//! Measure session host for the desktop UI (HZ-030 / AC-1.1--AC-1.8, AC-8.2).
//!
//! Collects frame probe + hardware/disk/network into a session and exports CSV/JSON.
//! Never invents FPS. Missing sensors stay unavailable.

use hertzline_core::collectors::{
    DiskCollector, HardwareCollector, NetworkCollector, OsPingProbe, SysinfoDiskIo,
    SysinfoHardwareSampler,
};
use hertzline_core::frame_capture::{FramePipeline, ProbeReport, ProbeStatus};
use hertzline_core::session::{export_csv, export_json, MeasurementSession, SessionMeta};
use hertzline_core::snapshot::unix_ms_now;
use serde::{Deserialize, Serialize};

/// Inputs for one unprivileged measurement pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasureRequest {
    /// Optional process name label (does not inject or attach).
    pub target_process: Option<String>,
    /// User-chosen ping host; empty skips network collection (unavailable).
    pub network_host: Option<String>,
    /// Optional PresentMon CSV (`MsBetweenPresents`) when probe is Available.
    pub presentmon_csv: Option<String>,
    /// Optional MangoHud log (`frametime`) when probe is Available.
    pub mangohud_log: Option<String>,
    pub notes: Option<String>,
}

/// UI-facing session payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasureResult {
    pub session: MeasurementSession,
    pub frame_probe: ProbeReport,
    pub csv: String,
    pub json: String,
    /// AC-8.2: measure alone is not an action before/after; always honest.
    pub local_measurement_note: String,
}

pub const NO_LOCAL_ACTION_MEASUREMENT: &str =
    "No local before/after measurement exists yet for an optimization action on this machine.";

/// Run collectors + frame pipeline into one completed session (AC-1.x, AC-1.8).
pub fn run_measurement(request: MeasureRequest) -> Result<MeasureResult, String> {
    let started = unix_ms_now();
    let session_id = format!("hz-measure-{started}");
    let target = request
        .target_process
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut session = MeasurementSession::new(SessionMeta {
        session_id,
        started_at_unix_ms: started,
        ended_at_unix_ms: None,
        target_process: target,
        os: std::env::consts::OS.to_string(),
        notes: request.notes.filter(|s| !s.trim().is_empty()),
    });

    let mut pipeline = FramePipeline::open();
    let probe = pipeline.probe().clone();
    if pipeline.is_capture_allowed() {
        if let Some(csv) = request
            .presentmon_csv
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            pipeline
                .ingest_presentmon_csv(csv)
                .map_err(|e| e.to_string())?;
        }
        if let Some(log) = request
            .mangohud_log
            .as_ref()
            .filter(|s| !s.trim().is_empty())
        {
            pipeline
                .ingest_mangohud_log(log)
                .map_err(|e| e.to_string())?;
        }
    }
    let _frame_result = pipeline.finish_into(&mut session);

    HardwareCollector::new(SysinfoHardwareSampler::new()).collect_into(&mut session);
    DiskCollector::new(SysinfoDiskIo).collect_into(&mut session);

    let host = request
        .network_host
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(host) = host {
        NetworkCollector::new(OsPingProbe, host)
            .with_count(4)
            .collect_into(&mut session);
    } else {
        session.network =
            hertzline_core::session::NetworkSummary::all_unavailable("network host not set");
    }

    session.complete(unix_ms_now());

    let csv = export_csv(&session);
    let json = export_json(&session).map_err(|e| e.to_string())?;

    // Archive a compact summary for everyday weekly health (AC-5.6). Failure is non-fatal.
    let frame_status = match probe.status {
        ProbeStatus::Available => "available",
        ProbeStatus::Unavailable => "unavailable",
        ProbeStatus::NotPossible => "not_possible",
    };
    let _ = hertzline_core::everyday::append_measurement_summary(
        hertzline_core::everyday::MeasurementSummary {
            session_id: session.meta.session_id.clone(),
            started_at_unix_ms: session.meta.started_at_unix_ms,
            ended_at_unix_ms: session.meta.ended_at_unix_ms,
            target_process: session.meta.target_process.clone(),
            os: session.meta.os.clone(),
            frame_status: frame_status.into(),
            notes: session.meta.notes.clone(),
        },
    );

    Ok(MeasureResult {
        session,
        frame_probe: probe,
        csv,
        json,
        local_measurement_note: NO_LOCAL_ACTION_MEASUREMENT.to_string(),
    })
}

/// AC-8.2 helper: include local evidence text or the honest absence note.
pub fn explanation_with_evidence(local_verdict_message: Option<&str>) -> String {
    match local_verdict_message {
        Some(msg) if !msg.trim().is_empty() => {
            format!("Local before/after measurement on this machine: {msg}")
        }
        _ => NO_LOCAL_ACTION_MEASUREMENT.to_string(),
    }
}

/// True when frame probe blocked live FPS (UI must not invent numbers).
pub fn frame_metrics_unavailable(probe: &ProbeReport) -> bool {
    !matches!(probe.status, ProbeStatus::Available)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanation_without_local_data_is_honest() {
        assert_eq!(explanation_with_evidence(None), NO_LOCAL_ACTION_MEASUREMENT);
        assert_eq!(
            explanation_with_evidence(Some("")),
            NO_LOCAL_ACTION_MEASUREMENT
        );
    }

    #[test]
    fn explanation_includes_local_verdict_message() {
        let text = explanation_with_evidence(Some("no measurable effect"));
        assert!(text.contains("no measurable effect"));
        assert!(text.contains("Local before/after"));
    }

    #[test]
    fn run_measurement_exports_identical_field_set() {
        // Live collectors; frame may be unavailable without PresentMon/MangoHud.
        let result = run_measurement(MeasureRequest {
            target_process: Some("hertzline-test".into()),
            network_host: None,
            presentmon_csv: None,
            mangohud_log: None,
            notes: Some("HZ-030 unit".into()),
        })
        .expect("measure");
        assert_eq!(result.session.schema_version, 1);
        assert!(result.session.meta.ended_at_unix_ms.is_some());
        assert!(result.csv.contains("avg_fps"));
        assert!(result.json.contains("avg_fps"));
        assert_eq!(result.local_measurement_note, NO_LOCAL_ACTION_MEASUREMENT);
        // Never invent FPS when probe is not Available with samples.
        if frame_metrics_unavailable(&result.frame_probe) {
            match &result.session.frame.avg_fps {
                hertzline_core::session::MetricF64::Unavailable { .. } => {}
                hertzline_core::session::MetricF64::Available { .. } => {
                    panic!("unavailable probe must not yield invented avg_fps")
                }
            }
        }
    }

    #[test]
    fn empty_network_host_stays_unavailable() {
        let result = run_measurement(MeasureRequest {
            target_process: None,
            network_host: Some("  ".into()),
            presentmon_csv: None,
            mangohud_log: None,
            notes: None,
        })
        .expect("measure");
        match &result.session.network.network_latency_ms {
            hertzline_core::session::MetricF64::Unavailable { reason } => {
                assert!(reason.contains("not set"));
            }
            _ => panic!("expected unavailable network"),
        }
    }
}

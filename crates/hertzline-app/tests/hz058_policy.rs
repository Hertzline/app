//! HZ-058 policy freeze: no fake FPS claims; anti-cheat overlay never injects.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use hertzline_app::{
    list_action_catalog, no_local_benchmark_explanation, overlay_overview, run_labeled_mock_pair,
};
use hertzline_core::overlay::OVERLAY_SETTINGS_ENV;

fn temp_dir(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "hertzline-app-hz058-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

const FORBIDDEN_ACTION_FRAGMENTS: &[&str] = &[
    "bcdedit",
    "disable_defender",
    "timer_resolution",
    "standby_list",
    "ram_cleaner",
    "tcp_optimizer",
    "fps_boost",
    "overclock",
    "undervolt",
    "inject",
];

#[test]
fn hz058_overlay_never_injects_and_defaults_off_for_eac() {
    let root = temp_dir("eac");
    let prev = std::env::var_os(OVERLAY_SETTINGS_ENV);
    std::env::set_var(OVERLAY_SETTINGS_ENV, root.join("overlay.json").as_os_str());

    let game = root.join("game");
    fs::create_dir_all(game.join("EasyAntiCheat")).expect("eac");

    let overview = overlay_overview(Some(game.to_str().unwrap())).expect("overview");
    assert!(!overview.injects_into_game);
    assert!(overview.anti_cheat_detected);
    assert!(!overview.decision_enabled);

    match prev {
        Some(v) => std::env::set_var(OVERLAY_SETTINGS_ENV, v),
        None => std::env::remove_var(OVERLAY_SETTINGS_ENV),
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hz058_local_verdict_allows_measured_gain_not_marketing_form() {
    let (_before, _after, verdict) = run_labeled_mock_pair(16.0, 16.0).expect("mock pair");
    assert!(verdict.fps_claim_allowed);
    assert!(verdict.message.contains("no measurable effect"));
    assert!(!verdict.explanation.to_lowercase().contains("+60 fps"));
    assert!(!contains_numeric_fps_claim(&verdict.message));
    assert!(!contains_numeric_fps_claim(&verdict.explanation));

    let no_local = no_local_benchmark_explanation();
    assert!(no_local.contains("No local before/after"));
}

#[test]
fn hz058_catalog_has_no_forbidden_action_ids() {
    let cat = list_action_catalog();
    assert!(cat.actions.len() >= 23);
    for action in &cat.actions {
        let id = action.id.to_lowercase();
        for frag in FORBIDDEN_ACTION_FRAGMENTS {
            assert!(
                !id.contains(frag),
                "forbidden_action id fragment {frag} in {}",
                action.id
            );
        }
        let blob = format!(
            "{} {} {} {}",
            action.explanation.what,
            action.explanation.expected_effect,
            action.explanation.risk,
            action.explanation.how_to_undo
        );
        assert!(
            !contains_numeric_fps_claim(&blob),
            "forbidden_action marketing FPS in {}",
            action.id
        );
    }
}

fn contains_numeric_fps_claim(text: &str) -> bool {
    let lower = text.to_lowercase();
    for (i, _) in lower.match_indices('+') {
        let rest = &lower[i + 1..];
        let digits: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if digits.is_empty() {
            continue;
        }
        let after = rest[digits.len()..].trim_start();
        let after = after.strip_prefix('%').unwrap_or(after).trim_start();
        if after.starts_with("fps") {
            return true;
        }
    }
    false
}

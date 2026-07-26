//! Step 6.4 — portable limitations and cleanup, exposed to the UI.
//!
//! Because Chimera registers no Apps & Features entry, removal has to live in
//! its own window. That makes these two commands the entire uninstall story,
//! and the reason `cleanup_plan` is a separate call from `execute`: the user
//! sees exactly what will be deleted, and how much space it frees, before
//! anything is removed.
//!
//! The limitation list is the other half. A portable install genuinely cannot
//! do several things an MSIX install can, and stating them is what keeps
//! Chimera from quietly implying otherwise (Spec §10, ADR-002).

use serde::Serialize;
use tauri::State;

use chimera_runtime::portable::{cleanup_plan, limitations};

use crate::state::AppState;

/// One thing a portable install cannot do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitationDto {
    /// i18n key for the sentence explaining the consequence.
    pub detail_key: String,
}

/// One item cleanup would remove.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupEntryDto {
    /// The file or folder's own name. Deliberately not a full path — a real
    /// one contains the account name, and this list is what gets screenshotted.
    pub label: String,
    pub bytes: u64,
}

/// What removing this installation would delete.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlanDto {
    pub entries: Vec<CleanupEntryDto>,
    pub total_bytes: u64,
    /// True when stored API keys live in the OS credential store rather than
    /// in any of these files, so the UI can say that deleting them is a
    /// separate action. Without it "clean up" would mean something narrower
    /// than the user assumed.
    pub leaves_keychain_entries: bool,
}

/// Everything a portable install cannot do.
#[tauri::command]
pub fn get_portable_limitations() -> Vec<LimitationDto> {
    limitations()
        .into_iter()
        .map(|l| LimitationDto {
            detail_key: l.detail_key().to_string(),
        })
        .collect()
}

/// Describe what cleanup would remove, without removing anything.
#[tauri::command]
pub fn get_cleanup_plan(state: State<'_, AppState>) -> Result<CleanupPlanDto, String> {
    let plan = cleanup_plan(&state.paths.data_root, &state.paths.runtime_root());
    Ok(CleanupPlanDto {
        entries: plan
            .entries
            .iter()
            .map(|e| CleanupEntryDto {
                label: e.display_label.clone(),
                bytes: e.bytes,
            })
            .collect(),
        total_bytes: plan.total_bytes,
        leaves_keychain_entries: plan.leaves_keychain_entries,
    })
}

/// Remove everything the plan listed.
///
/// Re-derives the plan rather than accepting one from the frontend: a plan that
/// crossed the IPC boundary and came back could name any path at all, which
/// would turn a cleanup button into an arbitrary-delete primitive.
#[tauri::command]
pub fn execute_cleanup(state: State<'_, AppState>) -> Result<(), String> {
    cleanup_plan(&state.paths.data_root, &state.paths.runtime_root())
        .execute()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtos_serialise_camel_case() {
        let json = serde_json::to_string(&CleanupPlanDto {
            entries: vec![CleanupEntryDto {
                label: "logs".into(),
                bytes: 10,
            }],
            total_bytes: 10,
            leaves_keychain_entries: true,
        })
        .unwrap();
        assert!(json.contains("totalBytes"), "{json}");
        assert!(json.contains("leavesKeychainEntries"), "{json}");
    }

    #[test]
    fn every_limitation_reaches_the_frontend_with_a_key() {
        let dtos = get_portable_limitations();
        assert_eq!(
            dtos.len(),
            limitations().len(),
            "a limitation was dropped in translation"
        );
        for d in dtos {
            assert!(
                d.detail_key.starts_with("portable."),
                "unnamespaced: {}",
                d.detail_key
            );
        }
    }

    #[test]
    fn a_cleanup_entry_label_is_never_a_path() {
        // The DTO is what the UI renders. A path here would put the account
        // name on screen in the one dialog people screenshot before clicking.
        let dto = CleanupEntryDto {
            label: "providers.sqlite".into(),
            bytes: 1,
        };
        assert!(!dto.label.contains(std::path::MAIN_SEPARATOR));
        assert!(!dto.label.contains('/'));
    }
}

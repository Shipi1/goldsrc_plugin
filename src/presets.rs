use crate::GoldsrcPluginParams;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PRESET_DIR_NAME: &str = "GoldSrc Presets";
const OVERRIDE_DIR_ENV: &str = "GOLDSRC_PRESETS_DIR";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct PluginParamsSnapshot {
    pub room: i32,
    pub reverb_mix: f32,
    pub delay_mix: f32,
    pub clip_soft: i32,
    pub enable_amplp: bool,
    pub enable_ampmod: bool,
    pub reverb_size: f32,
    pub reverb_feedback: f32,
    pub enable_revlp: bool,
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub enable_dellp: bool,
    pub haas_time: f32,
    pub seed: i32,
}

impl PluginParamsSnapshot {
    pub(crate) fn from_params(params: &GoldsrcPluginParams) -> Self {
        Self {
            room: params.room.value(),
            reverb_mix: params.reverb_mix.value(),
            delay_mix: params.delay_mix.value(),
            clip_soft: params.clip_soft.value(),
            enable_amplp: params.enable_amplp.value(),
            enable_ampmod: params.enable_ampmod.value(),
            reverb_size: params.reverb_size.value(),
            reverb_feedback: params.reverb_feedback.value(),
            enable_revlp: params.enable_revlp.value(),
            delay_time: params.delay_time.value(),
            delay_feedback: params.delay_feedback.value(),
            enable_dellp: params.enable_dellp.value(),
            haas_time: params.haas_time.value(),
            seed: params.seed.value(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PresetIoError {
    MissingDesktop,
    InvalidFileName,
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for PresetIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDesktop => {
                write!(f, "Could not determine Desktop path (USERPROFILE/HOME not set)")
            }
            Self::InvalidFileName => write!(f, "Preset file name is empty or invalid"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON parse/serialize error: {e}"),
        }
    }
}

impl std::error::Error for PresetIoError {}

impl From<io::Error> for PresetIoError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for PresetIoError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub(crate) fn save_params_snapshot(
    params: &GoldsrcPluginParams,
    name: &str,
) -> Result<PathBuf, PresetIoError> {
    let snapshot = PluginParamsSnapshot::from_params(params);
    save_snapshot(&snapshot, name)
}

pub(crate) fn save_snapshot(
    snapshot: &PluginParamsSnapshot,
    name: &str,
) -> Result<PathBuf, PresetIoError> {
    let path = snapshot_path(name)?;
    let json = serde_json::to_string_pretty(snapshot)?;
    fs::write(&path, json)?;
    println!("[presets] saved snapshot '{name}' to {}", path.display());
    Ok(path)
}

pub(crate) fn load_snapshot(name: &str) -> Result<PluginParamsSnapshot, PresetIoError> {
    let path = snapshot_path(name)?;
    println!("[presets] loading snapshot '{name}' from {}", path.display());
    load_snapshot_from_path(path)
}

pub(crate) fn load_snapshot_from_path(
    path: impl AsRef<Path>,
) -> Result<PluginParamsSnapshot, PresetIoError> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    let snapshot = serde_json::from_str::<PluginParamsSnapshot>(&json)?;
    println!("[presets] loaded snapshot from {}", path.display());
    Ok(snapshot)
}

pub(crate) fn list_snapshot_files() -> Result<Vec<PathBuf>, PresetIoError> {
    let dir = preset_root_dir()?;
    let mut entries = Vec::new();

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            entries.push(path);
        }
    }

    entries.sort();
    println!(
        "[presets] found {} snapshot file(s) in {}",
        entries.len(),
        dir.display()
    );
    Ok(entries)
}

pub(crate) fn preset_root_dir() -> Result<PathBuf, PresetIoError> {
    if let Some(override_dir) = env::var_os(OVERRIDE_DIR_ENV) {
        let path = PathBuf::from(override_dir);
        fs::create_dir_all(&path)?;
        return Ok(path);
    }

    let desktop = desktop_dir().ok_or(PresetIoError::MissingDesktop)?;
    let path = desktop.join(PRESET_DIR_NAME);
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn snapshot_path(name: &str) -> Result<PathBuf, PresetIoError> {
    let safe_name = sanitize_name(name);
    if safe_name.is_empty() {
        return Err(PresetIoError::InvalidFileName);
    }

    Ok(preset_root_dir()?.join(format!("{safe_name}.json")))
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());

    for ch in name.chars() {
        let invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid || ch.is_control() {
            out.push('_');
        } else {
            out.push(ch);
        }
    }

    out.trim().trim_matches('.').to_string()
}

fn desktop_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|p| p.join("Desktop"))
        .or_else(|| env::var_os("HOME").map(PathBuf::from).map(|p| p.join("Desktop")))
}

pub(crate) fn default_snapshot_name() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("preset-{now}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn snapshot_round_trip_json() {
        let original = PluginParamsSnapshot {
            room: 5,
            reverb_mix: 0.17,
            delay_mix: 0.25,
            clip_soft: 2,
            enable_amplp: false,
            enable_ampmod: true,
            reverb_size: 0.05,
            reverb_feedback: 0.85,
            enable_revlp: true,
            delay_time: 0.008,
            delay_feedback: 0.96,
            enable_dellp: true,
            haas_time: 0.01,
            seed: 42,
        };

        let json = serde_json::to_string_pretty(&original).expect("serialize snapshot");
        let restored: PluginParamsSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(restored, original);
    }

    #[test]
    fn save_and_load_snapshot_via_override_dir() {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("lock env mutex");

        let unique = format!(
            "goldsrc-presets-test-{}-{}",
            std::process::id(),
            default_snapshot_name()
        );
        let test_dir = std::env::temp_dir().join(unique);

        let _ = fs::remove_dir_all(&test_dir);
        std::env::set_var(OVERRIDE_DIR_ENV, &test_dir);

        let snapshot = PluginParamsSnapshot {
            room: 7,
            reverb_mix: 0.22,
            delay_mix: 0.31,
            clip_soft: 1,
            enable_amplp: true,
            enable_ampmod: false,
            reverb_size: 0.12,
            reverb_feedback: 0.73,
            enable_revlp: false,
            delay_time: 0.015,
            delay_feedback: 0.67,
            enable_dellp: false,
            haas_time: 0.02,
            seed: 99,
        };

        let saved_path = save_snapshot(&snapshot, "io_test").expect("save snapshot");
        assert!(saved_path.exists(), "snapshot file should exist");
        assert_eq!(saved_path.parent(), Some(test_dir.as_path()));

        let loaded = load_snapshot("io_test").expect("load snapshot");
        assert_eq!(loaded, snapshot);

        let listed = list_snapshot_files().expect("list snapshots");
        assert!(listed.contains(&saved_path));

        std::env::remove_var(OVERRIDE_DIR_ENV);
        let _ = fs::remove_dir_all(&test_dir);
    }
}






//! Application settings model and persistent configuration manager.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Theme preference for the application appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemePreference {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "System",
        }
    }
}

/// Preferred application language / locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppLanguage {
    #[default]
    Auto,
    En,
    ZhCn,
}

impl AppLanguage {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Auto => "System / 跟随系统",
            Self::En => "English",
            Self::ZhCn => "简体中文",
        }
    }

    /// Resolves auto language to concrete locale
    pub fn resolve_locale(&self) -> Self {
        match self {
            Self::Auto => {
                if let Ok(lang) = std::env::var("LANG") {
                    if lang.to_lowercase().starts_with("zh") {
                        return Self::ZhCn;
                    }
                }
                Self::En
            }
            other => *other,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Appearance and theme preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceSettings {
    pub theme: ThemePreference,
    #[serde(default = "default_true")]
    pub show_activity_bar: bool,
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    pub last_update_check: Option<String>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Dark,
            show_activity_bar: true,
            show_status_bar: true,
            last_update_check: None,
        }
    }
}

/// SQL and code editor preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EditorSettings {
    pub font_family: String,
    pub font_size: f32,
    pub tab_size: usize,
    pub line_numbers: bool,
    pub word_wrap: bool,
    pub format_on_run: bool,
    pub bracket_matching: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 13.0,
            tab_size: 4,
            line_numbers: true,
            word_wrap: true,
            format_on_run: false,
            bracket_matching: true,
        }
    }
}

/// Database query execution and safety preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuerySettings {
    pub default_limit: usize,
    pub query_timeout_secs: u64,
    pub safe_mode: bool,
    pub auto_explain_slow: bool,
    pub history_limit: usize,
}

impl Default for QuerySettings {
    fn default() -> Self {
        Self {
            default_limit: 1000,
            query_timeout_secs: 30,
            safe_mode: true,
            auto_explain_slow: false,
            history_limit: 500,
        }
    }
}

/// Top-level application settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppSettings {
    pub appearance: AppearanceSettings,
    pub editor: EditorSettings,
    pub query: QuerySettings,
    pub language: AppLanguage,
    #[serde(default)]
    pub last_connection_id: Option<String>,
}

/// Persistent settings manager loading from and saving to ~/.config/zqlcrab/settings.json
#[derive(Debug, Clone)]
pub struct SettingsManager {
    settings: AppSettings,
    storage_path: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Self {
        let storage_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zqlcrab")
            .join("settings.json");

        let mut mgr = Self {
            settings: AppSettings::default(),
            storage_path,
        };
        let _ = mgr.load();
        mgr
    }

    #[cfg(test)]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            settings: AppSettings::default(),
            storage_path: path,
        }
    }

    pub fn load(&mut self) -> Result<(), String> {
        if !self.storage_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.storage_path).map_err(|e| e.to_string())?;
        if let Ok(loaded) = serde_json::from_str::<AppSettings>(&content) {
            self.settings = loaded;
        }
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        fs::write(&self.storage_path, json).map_err(|e| e.to_string())
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut AppSettings {
        &mut self.settings
    }

    pub fn update<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut AppSettings),
    {
        f(&mut self.settings);
        self.save()
    }

    pub fn reset_defaults(&mut self) -> Result<(), String> {
        self.settings = AppSettings::default();
        self.save()
    }
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default_values() {
        let settings = AppSettings::default();
        assert_eq!(settings.appearance.theme, ThemePreference::Dark);
        assert_eq!(settings.editor.font_size, 13.0);
        assert_eq!(settings.editor.tab_size, 4);
        assert!(settings.query.safe_mode);
        assert_eq!(settings.language, AppLanguage::Auto);
    }

    #[test]
    fn test_settings_save_and_load() {
        let temp_dir = std::env::temp_dir().join(format!("zqlcrab_test_{}", uuid::Uuid::new_v4()));
        let config_path = temp_dir.join("settings.json");

        let mut mgr = SettingsManager::with_path(config_path.clone());
        mgr.update(|s| {
            s.appearance.theme = ThemePreference::Light;
            s.editor.font_size = 15.0;
            s.language = AppLanguage::ZhCn;
        })
        .expect("save should succeed");

        let mut mgr2 = SettingsManager::with_path(config_path.clone());
        mgr2.load().expect("load should succeed");
        assert_eq!(mgr2.settings().appearance.theme, ThemePreference::Light);
        assert_eq!(mgr2.settings().editor.font_size, 15.0);
        assert_eq!(mgr2.settings().language, AppLanguage::ZhCn);

        // Test reset defaults
        mgr2.reset_defaults().expect("reset should succeed");
        assert_eq!(mgr2.settings().appearance.theme, ThemePreference::Dark);
        assert_eq!(mgr2.settings().editor.font_size, 13.0);
        assert_eq!(mgr2.settings().language, AppLanguage::Auto);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_language_resolve_locale() {
        assert_eq!(AppLanguage::En.resolve_locale(), AppLanguage::En);
        assert_eq!(AppLanguage::ZhCn.resolve_locale(), AppLanguage::ZhCn);
        let auto_resolved = AppLanguage::Auto.resolve_locale();
        assert!(matches!(auto_resolved, AppLanguage::En | AppLanguage::ZhCn));
    }
}

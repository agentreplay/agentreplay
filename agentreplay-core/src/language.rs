// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Multi-Language Mode Support
//!
//! Support for generating observations in different languages
//! based on user preference or project configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported languages for observation generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// English (default)
    English,
    /// Spanish
    Spanish,
    /// French
    French,
    /// German
    German,
    /// Japanese
    Japanese,
    /// Chinese (Simplified)
    ChineseSimplified,
    /// Korean
    Korean,
    /// Portuguese
    Portuguese,
}

impl Default for Language {
    fn default() -> Self {
        Self::English
    }
}

impl Language {
    /// Get ISO 639-1 language code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Japanese => "ja",
            Self::ChineseSimplified => "zh",
            Self::Korean => "ko",
            Self::Portuguese => "pt",
        }
    }

    /// Get language name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Japanese => "日本語",
            Self::ChineseSimplified => "简体中文",
            Self::Korean => "한국어",
            Self::Portuguese => "Português",
        }
    }

    /// Parse from string (code or name).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Some(Self::English),
            "es" | "spanish" | "español" => Some(Self::Spanish),
            "fr" | "french" | "français" => Some(Self::French),
            "de" | "german" | "deutsch" => Some(Self::German),
            "ja" | "japanese" | "日本語" => Some(Self::Japanese),
            "zh" | "chinese" | "简体中文" => Some(Self::ChineseSimplified),
            "ko" | "korean" | "한국어" => Some(Self::Korean),
            "pt" | "portuguese" | "português" => Some(Self::Portuguese),
            _ => None,
        }
    }

    /// Get all supported languages.
    pub fn all() -> &'static [Language] {
        &[
            Self::English,
            Self::Spanish,
            Self::French,
            Self::German,
            Self::Japanese,
            Self::ChineseSimplified,
            Self::Korean,
            Self::Portuguese,
        ]
    }
}

/// Language-specific prompt templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePrompts {
    /// System prompt prefix for language instruction.
    pub system_prefix: String,
    /// Observation type labels.
    pub type_labels: HashMap<String, String>,
    /// Common phrases.
    pub phrases: HashMap<String, String>,
}

impl LanguagePrompts {
    /// Get prompts for a language.
    pub fn for_language(lang: Language) -> Self {
        match lang {
            Language::English => Self::english(),
            Language::Spanish => Self::spanish(),
            Language::French => Self::french(),
            Language::German => Self::german(),
            Language::Japanese => Self::japanese(),
            Language::ChineseSimplified => Self::chinese(),
            Language::Korean => Self::korean(),
            Language::Portuguese => Self::portuguese(),
        }
    }

    fn english() -> Self {
        Self {
            system_prefix: "Generate observations in English.".to_string(),
            type_labels: [
                ("implementation", "Implementation"),
                ("debugging", "Debugging"),
                ("refactoring", "Refactoring"),
                ("testing", "Testing"),
                ("architecture", "Architecture"),
                ("design", "Design"),
                ("research", "Research"),
                ("documentation", "Documentation"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "Title"),
                ("narrative", "Narrative"),
                ("facts", "Facts"),
                ("concepts", "Concepts"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn spanish() -> Self {
        Self {
            system_prefix: "Genera observaciones en español.".to_string(),
            type_labels: [
                ("implementation", "Implementación"),
                ("debugging", "Depuración"),
                ("refactoring", "Refactorización"),
                ("testing", "Pruebas"),
                ("architecture", "Arquitectura"),
                ("design", "Diseño"),
                ("research", "Investigación"),
                ("documentation", "Documentación"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "Título"),
                ("narrative", "Narrativa"),
                ("facts", "Hechos"),
                ("concepts", "Conceptos"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn french() -> Self {
        Self {
            system_prefix: "Générez des observations en français.".to_string(),
            type_labels: [
                ("implementation", "Implémentation"),
                ("debugging", "Débogage"),
                ("refactoring", "Refactorisation"),
                ("testing", "Tests"),
                ("architecture", "Architecture"),
                ("design", "Conception"),
                ("research", "Recherche"),
                ("documentation", "Documentation"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "Titre"),
                ("narrative", "Récit"),
                ("facts", "Faits"),
                ("concepts", "Concepts"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn german() -> Self {
        Self {
            system_prefix: "Generieren Sie Beobachtungen auf Deutsch.".to_string(),
            type_labels: [
                ("implementation", "Implementierung"),
                ("debugging", "Fehlersuche"),
                ("refactoring", "Refaktorisierung"),
                ("testing", "Tests"),
                ("architecture", "Architektur"),
                ("design", "Design"),
                ("research", "Forschung"),
                ("documentation", "Dokumentation"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "Titel"),
                ("narrative", "Beschreibung"),
                ("facts", "Fakten"),
                ("concepts", "Konzepte"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn japanese() -> Self {
        Self {
            system_prefix: "日本語で観察を生成してください。".to_string(),
            type_labels: [
                ("implementation", "実装"),
                ("debugging", "デバッグ"),
                ("refactoring", "リファクタリング"),
                ("testing", "テスト"),
                ("architecture", "アーキテクチャ"),
                ("design", "設計"),
                ("research", "調査"),
                ("documentation", "ドキュメント"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "タイトル"),
                ("narrative", "説明"),
                ("facts", "事実"),
                ("concepts", "概念"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn chinese() -> Self {
        Self {
            system_prefix: "请用简体中文生成观察记录。".to_string(),
            type_labels: [
                ("implementation", "实现"),
                ("debugging", "调试"),
                ("refactoring", "重构"),
                ("testing", "测试"),
                ("architecture", "架构"),
                ("design", "设计"),
                ("research", "研究"),
                ("documentation", "文档"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "标题"),
                ("narrative", "描述"),
                ("facts", "事实"),
                ("concepts", "概念"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn korean() -> Self {
        Self {
            system_prefix: "한국어로 관찰을 생성하세요.".to_string(),
            type_labels: [
                ("implementation", "구현"),
                ("debugging", "디버깅"),
                ("refactoring", "리팩토링"),
                ("testing", "테스트"),
                ("architecture", "아키텍처"),
                ("design", "설계"),
                ("research", "연구"),
                ("documentation", "문서화"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "제목"),
                ("narrative", "설명"),
                ("facts", "사실"),
                ("concepts", "개념"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }

    fn portuguese() -> Self {
        Self {
            system_prefix: "Gere observações em português.".to_string(),
            type_labels: [
                ("implementation", "Implementação"),
                ("debugging", "Depuração"),
                ("refactoring", "Refatoração"),
                ("testing", "Testes"),
                ("architecture", "Arquitetura"),
                ("design", "Design"),
                ("research", "Pesquisa"),
                ("documentation", "Documentação"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            phrases: [
                ("title", "Título"),
                ("narrative", "Narrativa"),
                ("facts", "Fatos"),
                ("concepts", "Conceitos"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        }
    }
}

/// Language configuration for a project or session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    /// Primary language for observations.
    pub primary: Language,
    /// Whether to detect language from user input.
    pub auto_detect: bool,
    /// Whether concepts should always be in English (for indexing).
    pub english_concepts: bool,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            primary: Language::English,
            auto_detect: false,
            english_concepts: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code() {
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::Japanese.code(), "ja");
    }

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("en"), Some(Language::English));
        assert_eq!(Language::from_str("japanese"), Some(Language::Japanese));
        assert_eq!(Language::from_str("日本語"), Some(Language::Japanese));
        assert_eq!(Language::from_str("invalid"), None);
    }

    #[test]
    fn test_language_prompts() {
        let prompts = LanguagePrompts::for_language(Language::Spanish);
        assert!(prompts.system_prefix.contains("español"));
        assert_eq!(
            prompts.type_labels.get("implementation"),
            Some(&"Implementación".to_string())
        );
    }

    #[test]
    fn test_all_languages_have_prompts() {
        for lang in Language::all() {
            let prompts = LanguagePrompts::for_language(*lang);
            assert!(!prompts.system_prefix.is_empty());
            assert!(!prompts.type_labels.is_empty());
        }
    }
}

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

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Inference,
    Embedding,
    Tooling,
    Storage,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: ProviderKind,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderFilter {
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
    pub kind: Option<ProviderKind>,
}

impl ProviderFilter {
    pub fn matches(&self, config: &ProviderConfig) -> bool {
        if let Some(kind) = &self.kind {
            if kind != &config.kind {
                return false;
            }
        }

        let caps: std::collections::HashSet<&str> =
            config.capabilities.iter().map(|c| c.as_str()).collect();
        for required in &self.required_capabilities {
            if !caps.contains(required.as_str()) {
                return false;
            }
        }
        true
    }
}

#[async_trait]
pub trait ProviderDriver: Send + Sync {
    fn id(&self) -> &str;
    fn config(&self) -> &ProviderConfig;
    fn is_ready(&self) -> bool {
        true
    }

    async fn start(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

#[async_trait]
pub trait ProviderFactory: Send + Sync {
    async fn load(&self, config: &ProviderConfig) -> Result<Arc<dyn ProviderDriver>, ProviderError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider not found: {0}")]
    NotFound(String),
    #[error("Provider already registered: {0}")]
    AlreadyRegistered(String),
    #[error("Provider load failed: {0}")]
    LoadFailed(String),
}

#[derive(Default)]
pub struct ProviderRegistry {
    drivers: RwLock<HashMap<String, Arc<dyn ProviderDriver>>>,
    configs: RwLock<HashMap<String, ProviderConfig>>,
    factories: RwLock<HashMap<String, Arc<dyn ProviderFactory>>>,
}

impl ProviderRegistry {
    pub fn register_driver(
        &self,
        config: ProviderConfig,
        driver: Arc<dyn ProviderDriver>,
    ) -> Result<(), ProviderError> {
        let provider_id = config.id.clone();
        let mut configs = self.configs.write();
        if configs.contains_key(&config.id) {
            return Err(ProviderError::AlreadyRegistered(config.id));
        }
        configs.insert(config.id.clone(), config);
        self.drivers
            .write()
            .insert(provider_id, driver);
        Ok(())
    }

    pub fn register_lazy(
        &self,
        config: ProviderConfig,
        factory: Arc<dyn ProviderFactory>,
    ) -> Result<(), ProviderError> {
        let provider_id = config.id.clone();
        let mut configs = self.configs.write();
        if configs.contains_key(&config.id) {
            return Err(ProviderError::AlreadyRegistered(config.id));
        }
        configs.insert(config.id.clone(), config);
        self.factories
            .write()
            .insert(provider_id, factory);
        Ok(())
    }

    pub fn unregister(&self, provider_id: &str) {
        self.configs.write().remove(provider_id);
        self.drivers.write().remove(provider_id);
        self.factories.write().remove(provider_id);
    }

    pub fn list_configs(&self) -> Vec<ProviderConfig> {
        self.configs.read().values().cloned().collect()
    }

    pub fn get_config(&self, provider_id: &str) -> Option<ProviderConfig> {
        self.configs.read().get(provider_id).cloned()
    }

    pub async fn get_or_load(
        &self,
        provider_id: &str,
    ) -> Result<Arc<dyn ProviderDriver>, ProviderError> {
        if let Some(driver) = self.drivers.read().get(provider_id) {
            return Ok(driver.clone());
        }

        let config = self
            .configs
            .read()
            .get(provider_id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(provider_id.to_string()))?;

        let factory = self
            .factories
            .read()
            .get(provider_id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound(provider_id.to_string()))?;

        let driver = factory
            .load(&config)
            .await
            .map_err(|err| ProviderError::LoadFailed(err.to_string()))?;
        self.drivers
            .write()
            .insert(provider_id.to_string(), driver.clone());
        Ok(driver)
    }

    pub fn find(&self, filter: &ProviderFilter) -> Vec<ProviderConfig> {
        self.configs
            .read()
            .values()
            .filter(|config| filter.matches(config))
            .cloned()
            .collect()
    }
}

use crate::config::{AppConfig, ModelConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDefinition {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub secret_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    models: Vec<ModelDefinition>,
}

impl ModelRegistry {
    pub fn from_config(config: &AppConfig) -> Self {
        let models = if config.ai.models.is_empty() {
            AppConfig::default().ai.models
        } else {
            config.ai.models.clone()
        };

        Self {
            models: models.into_iter().map(ModelDefinition::from).collect(),
        }
    }

    pub fn models(&self) -> &[ModelDefinition] {
        &self.models
    }

    pub fn selected_or_first(&self, selected_id: &str) -> Option<&ModelDefinition> {
        self.models
            .iter()
            .find(|model| model.id == selected_id)
            .or_else(|| self.models.first())
    }

    pub fn selected_label(&self, selected_id: &str) -> &str {
        self.selected_or_first(selected_id)
            .map(|model| model.label.as_str())
            .unwrap_or("No model")
    }

    pub fn normalized_selected_id(&self, selected_id: &str) -> Option<String> {
        self.selected_or_first(selected_id)
            .map(|model| model.id.clone())
    }

    pub fn next_after(&self, selected_id: &str) -> Option<&ModelDefinition> {
        if self.models.is_empty() {
            return None;
        }

        let current = self
            .models
            .iter()
            .position(|model| model.id == selected_id)
            .unwrap_or(0);
        let next = (current + 1) % self.models.len();
        self.models.get(next)
    }
}

impl From<ModelConfig> for ModelDefinition {
    fn from(config: ModelConfig) -> Self {
        Self {
            id: config.id,
            label: config.label,
            provider: config.provider,
            model: config.model,
            base_url: config.base_url,
            secret_ref: config.secret_ref,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModelRegistry;
    use crate::config::AppConfig;

    #[test]
    fn falls_back_to_first_model_when_selected_id_is_missing() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);

        let selected = registry
            .selected_or_first("missing")
            .expect("default model exists");

        assert_eq!(selected.id, "ollama-local");
    }

    #[test]
    fn cycles_models_from_selected_model() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);

        let next = registry
            .next_after("ollama-local")
            .expect("next model exists");

        assert_eq!(next.id, "openai-compatible");
    }

    #[test]
    fn empty_config_models_are_replaced_by_defaults() {
        let mut config = AppConfig::default();
        config.ai.models.clear();

        let registry = ModelRegistry::from_config(&config);

        assert_eq!(registry.models().len(), 2);
    }
}

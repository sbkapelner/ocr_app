use anyhow::anyhow;
use rten::Model;

/// Load a model from a local file path.
pub fn load_model(path: &str) -> Result<Model, anyhow::Error> {
    Model::load_file(path)
        .map_err(|e| anyhow!("Failed to load model from {}: {}", path, e))
}

pub mod api;
pub mod contract;
pub mod editor;
mod languages;
pub mod queue;
pub mod sandbox;
pub mod shutdown;
pub fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}
pub fn logging() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env("RUST_LOG", "practice=info"))
        .init();
}

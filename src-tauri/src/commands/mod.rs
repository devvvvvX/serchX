pub mod crawler;
pub mod llm;
pub mod onboarding;
pub mod search;
pub mod settings;
pub mod ui;
pub mod version;

pub use crawler::*;
pub use llm::*;
pub use onboarding::*;
pub use search::*;
pub use settings::*;
pub use ui::*;
pub use version::*;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to serchX.", name)
}

pub mod loader;
pub mod resolver;

pub use loader::{CampaignRules, CampaignSchema, LoadRulesError};
pub use resolver::CheckResult;

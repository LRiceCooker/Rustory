use serde_with;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Wording{
    #[serde(with = "serde_with::json::nested")]
    pub home_screen : PageContent
}
#[derive(Serialize, Deserialize)]
pub struct PageContent {
    pub title : String
}
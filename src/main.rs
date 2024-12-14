mod image;
mod ogp;
mod ui;

use ogp::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use ui::UI;

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(AppState::new()));
    let ui = UI::new(state);
    ui.run().await.unwrap();
}

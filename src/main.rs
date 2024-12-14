mod image;
mod ogp;
mod ui;

use clap::Parser;
use ogp::{AppState, fetch_ogp_info, normalize_url, OGPInfo};
use std::sync::Arc;
use tokio::sync::Mutex;
use ui::UI;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(default_value="")]
    url: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if !args.url.is_empty() {
        let client = reqwest::Client::new();
        match fetch_ogp_info(&client, &normalize_url(&args.url)).await {
            Ok(ogp_info) => print_ogp_info(&ogp_info),
            Err(e) => eprintln!("Error fetching OGP info: {}", e),
        }
    } else {
        let state = Arc::new(Mutex::new(AppState::new()));
        let ui = UI::new(state);
        ui.run().await.unwrap();
    }
}

fn print_ogp_info(ogp_info: &OGPInfo) {
    println!("Title: {}", ogp_info.title);
    println!("Description: {}", ogp_info.description);
    println!("Image URL: {}", ogp_info.image);
    println!("Metadata:");
    for meta in &ogp_info.metadata {
        println!("  - {}", meta);
    }
}

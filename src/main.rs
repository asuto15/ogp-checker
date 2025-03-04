mod image;
mod ogp;
mod ui;

use clap::Parser;
use ogp::{fetch_ogp_info, normalize_url, AppState, OGPInfo};
use std::sync::Arc;
use tokio::sync::Mutex;
use ui::UI;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(default_value = "")]
    url: String,

    #[arg(short, long)]
    json: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if !args.url.is_empty() {
        let client = reqwest::Client::new();
        match fetch_ogp_info(&client, &normalize_url(&args.url)).await {
            Ok(ogp_info) => {
                if args.json {
                    match serde_json::to_string_pretty(&ogp_info) {
                        Ok(json) => println!("{}", json),
                        Err(e) => eprintln!("Error serializing OGP info: {}", e),
                    }
                } else {
                    print_ogp_info(&ogp_info);
                }
            }
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
    for (tag, content) in &ogp_info.metadata {
        println!("\"{}\" - \"{}\"", tag, content);
    }
}

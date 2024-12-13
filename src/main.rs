mod image;
mod ogp;

use ogp::display_ogp;

#[tokio::main]
async fn main() {
    display_ogp().await;
}

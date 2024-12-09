use reqwest::blocking;
use scraper::{Html, Selector};
use viuer::Config;
use image::load_from_memory;

pub struct OGPInfo {
    title: String,
    description: String,
    image: String,
    metadata: Vec<String>,
}

fn fetch_ogp_info(url: &str) -> OGPInfo {
    let rc = blocking::get(url).unwrap();
    let contents = rc.text().unwrap();

    let document = Html::parse_document(&contents);
    let metadata_selector = Selector::parse("meta").unwrap();
    let title_selector = Selector::parse("meta[property='og:title']").unwrap();
    let description_selector = Selector::parse("meta[property='og:description']").unwrap();
    let image_selector = Selector::parse("meta[property='og:image']").unwrap();

    let title = document.select(&title_selector).next().unwrap().value().attr("content").unwrap();
    let description = document.select(&description_selector).next().unwrap().value().attr("content").unwrap();
    let image = document.select(&image_selector).next().unwrap().value().attr("content").unwrap();
    let metadata = document.select(&metadata_selector).filter_map(|element| element.value().attr("content")).collect::<Vec<_>>();

    OGPInfo {
        title: title.to_string(),
        description: description.to_string(),
        image: image.to_string(),
        metadata: metadata.iter().map(|s| s.to_string()).collect(),
    }
}

fn display_ogp_image(image_url: &str) {
    let image_response = blocking::get(image_url).unwrap();
    let image_bytes = image_response.bytes().unwrap();

    let image = load_from_memory(&image_bytes).unwrap();

    let conf = Config {
        width: Some(40),
        height: None,
        ..Default::default()
    };

    viuer::print(&image, &conf).unwrap();
}

pub fn display_ogp(url: &str) {
    let info = fetch_ogp_info(url);
    print!("                                        ");
    println!("Title: {}", info.title);
    print!("                                        ");
    println!("Description: {}", info.description);
    print!("                                        ");
    println!("OG Image: {}", info.image);
    display_ogp_image(&info.image);
    println!("Metadata: {:?}", info.metadata);
}
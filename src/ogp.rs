use crate::image::Image;
use image::DynamicImage;
use reqwest::Client;
use scraper::{Html, Selector};
use std::{io, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct OGPInfo {
    pub title: String,
    pub description: String,
    pub image: String,
    pub metadata: Vec<(String, String)>,
}

pub struct AppState {
    pub url: String,
    pub cursor_position: usize,
    pub ogp_info: Option<OGPInfo>,
    pub cached_image: Option<Image>,
    pub error_message: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            cursor_position: 0,
            ogp_info: None,
            cached_image: None,
            error_message: None,
        }
    }
}

pub fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    }
}

pub async fn update_ogp(state: Arc<Mutex<AppState>>, client: Client) {
    let url;
    {
        let state = state.lock().await;
        url = normalize_url(&state.url);
    }

    let ogp_result = fetch_ogp_info(&client, &url).await;
    let dynamic_img_result = if let Ok(ref ogp_info) = ogp_result {
        fetch_dynamic_image(&client, &ogp_info.image).await.ok()
    } else {
        None
    };

    let mut state = state.lock().await;
    match ogp_result {
        Ok(ogp_info) => {
            state.ogp_info = Some(ogp_info);
            state.cached_image = dynamic_img_result.map(Image::from_dynamic_image);
            state.error_message = None;
        }
        Err(err) => {
            state.error_message = Some(format!("Failed to fetch OGP info: {}", err));
        }
    }
}

pub async fn fetch_ogp_info(client: &Client, url: &str) -> Result<OGPInfo, reqwest::Error> {
    let res = client.get(url).send().await?.text().await?;
    let document = Html::parse_document(&res);

    let title = document
        .select(&Selector::parse("meta[property='og:title']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();

    let description = document
        .select(&Selector::parse("meta[property='og:description']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();

    let image = document
        .select(&Selector::parse("meta[property='og:image']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();

    let metadata = document
        .select(&Selector::parse("meta").unwrap())
        .filter_map(|e| {
            let tag = e
                .value()
                .attr("property")
                .or_else(|| e.value().attr("name"))
                .unwrap_or("")
                .to_string();
            let content = e.value().attr("content").unwrap_or("").to_string();
            if !tag.is_empty() {
                Some((tag, content))
            } else {
                None
            }
        })
        .collect();

    Ok(OGPInfo {
        title,
        description,
        image,
        metadata,
    })
}

async fn fetch_dynamic_image(client: &Client, url: &str) -> Result<DynamicImage, io::Error> {
    let res = client.get(url).send().await.map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("HTTP request failed: {}", err),
        )
    })?;
    let bytes = res.bytes().await.map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to read response body: {}", err),
        )
    })?;

    image::load_from_memory(&bytes).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported image format: {}", err),
        )
    })
}

use crate::image::Image;
use reqwest::{self, Client};
use scraper::{Html, Selector};
use image::DynamicImage;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Style, Color},
    widgets::{Block, Borders, Paragraph, canvas::{Canvas, Rectangle}},
    Terminal,
};
use std::{sync::Arc, io};
use tokio::sync::{Mutex, watch};

#[derive(Clone)]
pub struct OGPInfo {
    pub title: String,
    pub description: String,
    pub image: String,
    pub metadata: Vec<String>,
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

    pub fn normalize_url(&self) -> String {
        if self.url.starts_with("http://") || self.url.starts_with("https://") {
            self.url.clone()
        } else {
            format!("http://{}", self.url)
        }
    }
}

pub async fn update_ogp(state: Arc<Mutex<AppState>>, client: Client) {
    let url;
    {
        let state = state.lock().await;
        url = state.normalize_url();
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

pub async fn display_ogp() {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Arc::new(tokio::sync::Mutex::new(Terminal::new(backend).unwrap()));

    let state = Arc::new(tokio::sync::Mutex::new(AppState::new()));
    let client = Client::new();

    let (tx, rx) = watch::channel(());
    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    let mut needs_redraw = true;

    let rx_clone = Arc::clone(&rx);
    let state_clone = Arc::clone(&state);
    let terminal_clone = Arc::clone(&terminal);

    tokio::spawn(async move {
        loop {
            if needs_redraw || rx_clone.lock().await.changed().await.is_ok() {
                needs_redraw = false;

                let state = state_clone.lock().await;
                let mut terminal = terminal_clone.lock().await;

                if let Err(e) = terminal.draw(|f| {
                    let size = f.area();
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3),
                            Constraint::Length(3),
                            Constraint::Percentage(94),
                        ])
                        .split(size);

                    let mut url_display = state.url.clone();
                    if state.cursor_position <= state.url.len() {
                        url_display.insert(state.cursor_position, '|');
                    }

                    let url_input = Paragraph::new(url_display)
                        .block(Block::default().borders(Borders::ALL).title("Enter URL"))
                        .style(Style::default());
                    f.render_widget(url_input, chunks[0]);

                    if let Some(error_message) = &state.error_message {
                        let error_paragraph = Paragraph::new(error_message.clone())
                            .block(Block::default().borders(Borders::ALL).title("Error"))
                            .style(Style::default().fg(Color::Red));
                        f.render_widget(error_paragraph, chunks[1]);
                    }

                    let content_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Percentage(33),
                            Constraint::Percentage(67),
                        ])
                        .split(chunks[2]);

                    if let Some(info) = &state.ogp_info {
                        let meta_info = format!(
                            "Title: {}\nDescription: {}\nImage: {}\nMetadata: {} items",
                            info.title, info.description, info.image, info.metadata.len()
                        );

                        let meta_paragraph = Paragraph::new(meta_info)
                            .block(Block::default().borders(Borders::ALL).title("OGP Info"));
                        f.render_widget(meta_paragraph, content_chunks[1]);

                        if let Some(cached_image) = &state.cached_image {
                            draw_image_with_colors(f, content_chunks[0], cached_image);
                        } else {
                            let empty_paragraph = Paragraph::new("No image available")
                                .block(Block::default().borders(Borders::ALL).title("Image"));
                            f.render_widget(empty_paragraph, content_chunks[0]);
                        }
                    }
                }) {
                    eprintln!("Error drawing terminal: {}", e);
                }
            }
        }
    });

    loop {
        if let Event::Key(key) = event::read().unwrap() {
            let mut should_update = false;

            match key.code {
                KeyCode::Char(c) => {
                    let mut state = state.lock().await;
                    let cursor_position = state.cursor_position;
                    state.url.insert(cursor_position, c);
                    state.cursor_position += 1;
                    should_update = true;
                }
                KeyCode::Backspace => {
                    let mut state = state.lock().await;
                    let cursor_position = state.cursor_position;
                    if cursor_position > 0 {
                        state.url.remove(cursor_position - 1);
                        state.cursor_position -= 1;
                        should_update = true;
                    }
                }
                KeyCode::Left => {
                    let mut state = state.lock().await;
                    if state.cursor_position > 0 {
                        state.cursor_position -= 1;
                        should_update = true;
                    }
                }
                KeyCode::Right => {
                    let mut state = state.lock().await;
                    if state.cursor_position < state.url.len() {
                        state.cursor_position += 1;
                        should_update = true;
                    }
                }
                KeyCode::Enter => {
                    let url_is_empty;
                    {
                        let mut state = state.lock().await;
                        url_is_empty = state.url.is_empty();

                        if url_is_empty {
                            state.ogp_info = None;
                            state.cached_image = None;
                            state.error_message = None;
                        }
                    }
                    if !url_is_empty {
                        let state_clone = Arc::clone(&state);
                        let client_clone = client.clone();
                        let tx_clone = tx.clone();

                        tokio::spawn(async move {
                            update_ogp(state_clone, client_clone).await;
                            let _ = tx_clone.send(());
                        });
                    } else {
                        let _ = tx.send(());
                    }
                }
                KeyCode::Esc => break,
                _ => {}
            }

            if should_update {
                let _ = tx.send(());
            }
        }
    }

    disable_raw_mode().unwrap();
    let mut terminal = terminal.lock().await;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
    terminal.show_cursor().unwrap();
}

async fn fetch_ogp_info(client: &Client, url: &str) -> Result<OGPInfo, reqwest::Error> {
    let res = client.get(url).send().await?.text().await?;
    let document = Html::parse_document(&res);
    let title = document.select(&Selector::parse("meta[property='og:title']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();
    let description = document.select(&Selector::parse("meta[property='og:description']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();
    let image = document.select(&Selector::parse("meta[property='og:image']").unwrap())
        .next()
        .and_then(|e| e.value().attr("content"))
        .unwrap_or("")
        .to_string();
    let metadata = document.select(&Selector::parse("meta").unwrap())
        .filter_map(|e| e.value().attr("content"))
        .map(|s| s.to_string())
        .collect();

    Ok(OGPInfo { title, description, image, metadata })
}

async fn fetch_dynamic_image(client: &Client, url: &str) -> Result<DynamicImage, io::Error> {
    let res = client.get(url).send().await.map_err(|err| {
        io::Error::new(io::ErrorKind::Other, format!("HTTP request failed: {}", err))
    })?;
    let bytes = res.bytes().await.map_err(|err| {
        io::Error::new(io::ErrorKind::Other, format!("Failed to read response body: {}", err))
    })?;

    image::load_from_memory(&bytes).map_err(|err| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Unsupported image format: {}", err))
    })
}

fn draw_image_with_colors(
    f: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    img: &Image,
) {
    let (target_width, target_height) = (area.width as usize, area.height as usize);

    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::ALL).title("Image"))
        .paint(|ctx| {
            for y in 0..target_height {
                for x in 0..target_width {
                    let src_x = x * (img.width as usize) / target_width;
                    let src_y = y * (img.height as usize) / target_height;
                    let idx = src_y * (img.width as usize) + src_x;

                    let (r, g, b) = img.pixels[idx];
                    let color = Color::Rgb(r, g, b);

                    ctx.draw(&Rectangle {
                        x: x as f64,
                        y: (target_height - 1 - y) as f64,
                        width: 1.0,
                        height: 1.0,
                        color,
                    });
                }
            }
        })
        .x_bounds([0.0, target_width as f64])
        .y_bounds([0.0, target_height as f64]);

    f.render_widget(canvas, area);
}

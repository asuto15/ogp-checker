use crate::image::Image;
use reqwest::blocking;
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
use std::io;

pub struct OGPInfo {
    pub title: String,
    pub description: String,
    pub image: String,
    pub metadata: Vec<String>,
}

pub struct AppState {
    pub url: String,
    pub ogp_info: Option<OGPInfo>,
    pub cached_image: Option<Image>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            url: String::new(),
            ogp_info: None,
            cached_image: None,
        }
    }

    pub fn update_ogp(&mut self) {
        if let Ok(info) = fetch_ogp_info(&self.url) {
            let image_url = info.image.clone();
            if let Ok(dynamic_img) = fetch_dynamic_image(&image_url) {
                self.cached_image = Some(Image::from_dynamic_image(&dynamic_img));
            }
            self.ogp_info = Some(info);
        }
    }
}

pub fn display_ogp() {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = AppState::new();

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Percentage(97),
                ])
                .split(size);

            let url_input = Paragraph::new(app.url.clone())
                .block(Block::default().borders(Borders::ALL).title("Enter URL"))
                .style(Style::default());
            f.render_widget(url_input, chunks[0]);

            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(67),
                ])
                .split(chunks[1]);

            if let Some(info) = &app.ogp_info {
                let meta_info = format!(
                    "Title: {}\nDescription: {}\nImage: {}\nMetadata: {} items",
                    info.title, info.description, info.image, info.metadata.len()
                );

                let meta_paragraph = Paragraph::new(meta_info)
                    .block(Block::default().borders(Borders::ALL).title("OGP Info"));
                f.render_widget(meta_paragraph, content_chunks[1]);

                if let Some(cached_image) = &app.cached_image {
                    draw_image_with_colors(f, content_chunks[0], cached_image);
                }
            }
        }).unwrap();

        if let Event::Key(key) = event::read().unwrap() {
            match key.code {
                KeyCode::Char(c) => app.url.push(c),
                KeyCode::Backspace => { app.url.pop(); },
                KeyCode::Enter => { app.update_ogp(); },
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
    terminal.show_cursor().unwrap();
}

fn fetch_ogp_info(url: &str) -> Result<OGPInfo, reqwest::Error> {
    let rc = blocking::get(url)?;
    let contents = rc.text()?;

    let document = Html::parse_document(&contents);
    let metadata_selector = Selector::parse("meta").unwrap();
    let title_selector = Selector::parse("meta[property='og:title']").unwrap();
    let description_selector = Selector::parse("meta[property='og:description']").unwrap();
    let image_selector = Selector::parse("meta[property='og:image']").unwrap();

    let title = document
        .select(&title_selector)
        .next()
        .and_then(|t| t.value().attr("content"))
        .unwrap_or_default()
        .to_string();

    let description = document
        .select(&description_selector)
        .next()
        .and_then(|d| d.value().attr("content"))
        .unwrap_or_default()
        .to_string();

    let image = document
        .select(&image_selector)
        .next()
        .and_then(|i| i.value().attr("content"))
        .unwrap_or_default()
        .to_string();

    let metadata = document
        .select(&metadata_selector)
        .filter_map(|m| m.value().attr("content"))
        .map(|s| s.to_string())
        .collect();

    Ok(OGPInfo {
        title,
        description,
        image,
        metadata,
    })
}

fn fetch_dynamic_image(url: &str) -> Result<DynamicImage, reqwest::Error> {
    let response = blocking::get(url)?;
    let bytes = response.bytes()?;
    Ok(image::load_from_memory(&bytes).unwrap())
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

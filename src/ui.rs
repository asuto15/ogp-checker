use crate::{
    image::Image,
    ogp::{update_ogp, AppState},
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, Borders, Paragraph,
    },
    Terminal,
};
use std::{io, sync::Arc};
use tokio::sync::{watch, Mutex};

pub struct UI {
    state: Arc<Mutex<AppState>>,
    tx: watch::Sender<()>,
}

impl UI {
    pub fn new(state: Arc<Mutex<AppState>>) -> Self {
        let (tx, _) = watch::channel(());
        UI { state, tx }
    }

    pub async fn run(&self) -> Result<(), io::Error> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Arc::new(Mutex::new(Terminal::new(backend)?));

        let rx = self.tx.subscribe();
        let state_clone = Arc::clone(&self.state);
        let terminal_clone = Arc::clone(&terminal);

        self.tx.send(()).unwrap();

        tokio::spawn(async move {
            let mut rx = rx;
            loop {
                rx.changed().await.unwrap();
                let state = state_clone.lock().await;
                let mut terminal = terminal_clone.lock().await;
                terminal
                    .draw(|f| UI::draw_ui(f, &state))
                    .expect("Failed to draw UI");
            }
        });

        loop {
            if let Event::Key(key_event) = event::read()? {
                if self.handle_input(key_event.code).await {
                    break;
                }
            }
        }

        disable_raw_mode()?;
        let mut terminal = terminal.lock().await;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn draw_ui(f: &mut ratatui::Frame, state: &AppState) {
        let size = f.area();
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Percentage(40),
                Constraint::Percentage(60),
            ])
            .split(size);

        let image_and_info_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(2, 3),
            ])
            .split(vertical_chunks[1]);

        let mut url_display = state.url.clone();
        if state.cursor_position <= state.url.len() {
            url_display.insert(state.cursor_position, '|');
        }

        let url_input = Paragraph::new(url_display)
            .block(Block::default().borders(Borders::ALL).title("Enter URL"))
            .style(Style::default());
        f.render_widget(url_input, vertical_chunks[0]);

        if let Some(error_message) = &state.error_message {
            let error_paragraph = Paragraph::new(error_message.clone())
                .block(Block::default().borders(Borders::ALL).title("Error"))
                .style(Style::default().fg(Color::Red));
            f.render_widget(error_paragraph, image_and_info_chunks[1]);
        } else if let Some(info) = &state.ogp_info {
            let ogp_info_display = format!(
                "Title: {}\nDescription: {}\nImage URL: {}\nMetadata Count: {}",
                info.title,
                info.description,
                info.image,
                info.metadata.len()
            );

            let ogp_info_paragraph = Paragraph::new(ogp_info_display)
                .block(Block::default().borders(Borders::ALL).title("OGP Info"))
                .style(Style::default());
            f.render_widget(ogp_info_paragraph, image_and_info_chunks[1]);

            if let Some(cached_image) = &state.cached_image {
                UI::draw_image_with_colors(f, image_and_info_chunks[0], cached_image);
            } else {
                let empty_paragraph = Paragraph::new("No image available")
                    .block(Block::default().borders(Borders::ALL).title("Image"));
                f.render_widget(empty_paragraph, image_and_info_chunks[0]);
            }
        }

        if let Some(info) = &state.ogp_info {
            let metadata_to_display = info
                .metadata
                .iter()
                .skip(state.metadata_offset)
                .take((vertical_chunks[2].height - 2) as usize)
                .map(|(tag, content)| format!("{}: {}", tag, content))
                .collect::<Vec<_>>()
                .join("\n");

            let metadata_paragraph = Paragraph::new(metadata_to_display)
                .block(Block::default().borders(Borders::ALL).title("Metadata"))
                .style(Style::default());
            f.render_widget(metadata_paragraph, vertical_chunks[2]);
        }
    }

    fn draw_image_with_colors(f: &mut ratatui::Frame, area: ratatui::layout::Rect, img: &Image) {
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

    async fn handle_input(&self, key: KeyCode) -> bool {
        let mut state = self.state.lock().await;
        match key {
            KeyCode::Char(c) => {
                let cursor_position = state.cursor_position;
                state.url.insert(cursor_position, c);
                state.cursor_position += 1;
                self.tx.send(()).unwrap();
            }
            KeyCode::Backspace => {
                let cursor_position = state.cursor_position;
                if cursor_position > 0 {
                    state.url.remove(cursor_position - 1);
                    state.cursor_position -= 1;
                    self.tx.send(()).unwrap();
                }
            }
            KeyCode::Left => {
                if state.cursor_position > 0 {
                    state.cursor_position -= 1;
                    self.tx.send(()).unwrap();
                }
            }
            KeyCode::Right => {
                if state.cursor_position < state.url.len() {
                    state.cursor_position += 1;
                    self.tx.send(()).unwrap();
                }
            }
            KeyCode::Up => {
                if state.metadata_offset > 0 {
                    state.metadata_offset -= 1;
                    self.tx.send(()).unwrap();
                }
            }
            KeyCode::Down => {
                if let Some(info) = &state.ogp_info {
                    if state.metadata_offset + 1 < info.metadata.len() {
                        state.metadata_offset += 1;
                        self.tx.send(()).unwrap();
                    }
                }
            }
            KeyCode::Enter => {
                if state.url.is_empty() {
                    state.ogp_info = None;
                    state.cached_image = None;
                    state.error_message = None;
                    self.tx.send(()).unwrap();
                } else {
                    let state_clone = Arc::clone(&self.state);
                    let tx_clone = self.tx.clone();
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        update_ogp(state_clone, client).await;
                        tx_clone.send(()).unwrap();
                    });
                }
            }
            KeyCode::Esc => return true,
            _ => {}
        }
        false
    }
}

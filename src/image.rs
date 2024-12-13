use image::{DynamicImage, GenericImageView};

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<(u8, u8, u8)>,
}

impl Image {
    pub fn from_dynamic_image(img: &DynamicImage) -> Self {
        let (width, height) = img.dimensions();
        let pixels = img
            .pixels()
            .map(|(_, _, p)| (p[0], p[1], p[2]))
            .collect();

        Image {
            width,
            height,
            pixels,
        }
    }
}

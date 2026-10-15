//! A tile's pixels, read and written as crawl's tile build reads and writes them
//! (`rltiles/tool/tile.cc` and `tile_colour.cc`), for the changes the build makes to them.

use png::{BitDepth, ColorType, Decoder, Encoder, Transformations};

/// The colour crawl's build outlines tiles with: `tile_colour::black`, opaque black.
const OUTLINE: [u8; 4] = [0, 0, 0, 255];

/// An image as 8-bit RGBA pixels, row by row.
pub struct Image {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
}

impl Image {
    /// Reads a PNG as `tile::load` does: palettes, greys and 16-bit samples become 8-bit RGBA,
    /// with an opaque alpha where the file has none.
    pub fn decode(png: &[u8]) -> Result<Self, png::DecodingError> {
        let mut decoder = Decoder::new(std::io::Cursor::new(png));
        decoder
            .set_transformations(Transformations::normalize_to_color8() | Transformations::ALPHA);
        let mut reader = decoder.read_info()?;
        let mut buffer = vec![0; reader.output_buffer_size().unwrap_or_default()];
        let frame = reader.next_frame(&mut buffer)?;
        let samples = buffer.get(..frame.buffer_size()).unwrap_or_default();
        let pixels = match frame.color_type {
            ColorType::Rgba => samples.as_chunks::<4>().0.to_vec(),
            ColorType::Rgb => samples
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            ColorType::GrayscaleAlpha => samples
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[g, a]| [g, g, g, a])
                .collect(),
            // The transformations leave no palette, so `Indexed` only reaches here as greys.
            ColorType::Grayscale | ColorType::Indexed => {
                samples.iter().map(|&g| [g, g, g, 255]).collect()
            }
        };
        Ok(Self {
            width: frame.width as usize,
            height: frame.height as usize,
            pixels,
        })
    }

    /// Outlines the image in black as `tile::add_rim` does: every pixel that isn't drawn (or is
    /// already the outline's black) and is beside a drawn pixel, up, down, left or right,
    /// becomes opaque black. Drawn pixels keep their colour, so the image stays whole pixels.
    pub fn outline(&mut self) {
        let drawn: Vec<bool> = self
            .pixels
            .iter()
            .map(|pixel| pixel[3] > 0 && *pixel != OUTLINE)
            .collect();
        let at = |x: usize, y: usize| drawn.get(y * self.width + x).copied().unwrap_or(false);
        for y in 0..self.height {
            for x in 0..self.width {
                let beside_drawn = (x > 0 && at(x - 1, y))
                    || (y > 0 && at(x, y - 1))
                    || (x + 1 < self.width && at(x + 1, y))
                    || (y + 1 < self.height && at(x, y + 1));
                if !at(x, y)
                    && beside_drawn
                    && let Some(pixel) = self.pixels.get_mut(y * self.width + x)
                {
                    *pixel = OUTLINE;
                }
            }
        }
    }

    /// The image as an 8-bit RGBA PNG.
    pub fn encode(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut png = Vec::new();
        let size = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let mut encoder = Encoder::new(&mut png, size(self.width), size(self.height));
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(self.pixels.as_flattened())?;
        writer.finish()?;
        Ok(png)
    }
}

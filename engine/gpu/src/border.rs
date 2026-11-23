//! the picture drawn behind the screens, filling the bars a screen's shape
//! leaves on a panel of another: the stock one, made at the panel's size,
//! or a PNG.

use std::path::Path;

/// what goes behind the screens.
pub enum Border {
    /// the one that comes with Pocket Native, made to fit each panel.
    Stock,
    /// a picture, RGBA premultiplied by alpha, rows from the top, covering
    /// the panel at its own shape, cut down at the sides or the top and
    /// bottom to the panel's.
    Picture { pixels: Vec<u8>, width: u32, height: u32 },
}

/// the biggest picture taken, either way: 4K covers any handheld or phone.
const MAX_SIZE: u32 = 4096;

/// a PNG as a border.
pub fn load_border(path: &Path) -> Result<Border, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut decoder = png::Decoder::new_with_limits(std::io::BufReader::new(file), png::Limits { bytes: 1 << 28 });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_SIZE || height > MAX_SIZE {
        return Err(format!("it is {width}x{height}, a border can be up to {MAX_SIZE}x{MAX_SIZE}"));
    }
    let size = reader.output_buffer_size().ok_or("it is too big")?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer).map_err(|error| error.to_string())?;
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("its palette was not expanded".to_owned()),
    };
    let (w, h) = (width as usize, height as usize);
    let mut pixels = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let line = &buffer[row * info.line_size..][..w * channels];
        match channels {
            4 => line.as_chunks::<4>().0.iter().for_each(|&[r, g, b, a]| pixels.extend_from_slice(&premultiplied([r, g, b], a))),
            3 => line.as_chunks::<3>().0.iter().for_each(|&[r, g, b]| pixels.extend_from_slice(&[r, g, b, 255])),
            2 => line.as_chunks::<2>().0.iter().for_each(|&[i, a]| pixels.extend_from_slice(&premultiplied([i, i, i], a))),
            _ => line.iter().for_each(|&i| pixels.extend_from_slice(&[i, i, i, 255])),
        }
    }
    Ok(Border::Picture { pixels, width, height })
}

fn premultiplied([r, g, b]: [u8; 3], a: u8) -> [u8; 4] {
    let times = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
    [times(r), times(g), times(b), a]
}

/// the stock border at a panel's size, opaque: a deep slate that lightens
/// a little towards the middle, behind the screens, under fine diagonal
/// lines, darker at the edges. Made from nothing, the same on every panel's
/// shape, sharp at any size.
pub fn stock_border(width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    // the lines' spacing follows the panel's size, about 90 across a
    // handheld's short side
    let spacing = (w.min(h) / 90.0).max(3.0);
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let v = y as f32 / h;
        for x in 0..width {
            let u = x as f32 / w;
            // darker towards the corners, by distance from the middle with
            // the panel's shape taken out
            let (dx, dy) = (u - 0.5, v - 0.5);
            let edge = (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            let glow = 1.0 - edge * edge * 0.55;
            // a fine line every few pixels, a soft one, at 45 degrees
            let phase = ((x as f32 + y as f32) / spacing).fract();
            let line = 1.0 + 0.06 * (1.0 - ((phase - 0.5).abs() * 4.0).min(1.0));
            // slate, a touch bluer at the top
            let base = [24.0 + 6.0 * (1.0 - v), 28.0 + 6.0 * (1.0 - v), 38.0 + 10.0 * (1.0 - v)];
            let shade = glow * line;
            pixels.extend_from_slice(&[(base[0] * shade) as u8, (base[1] * shade) as u8, (base[2] * shade) as u8, 255]);
        }
    }
    pixels
}

/// the part of a picture that covers a panel at its shape, centred: the
/// texture coordinates of its left, top, right and bottom.
pub fn covering((width, height): (u32, u32), (panel_width, panel_height): (i32, i32)) -> [f32; 4] {
    let picture = width as f32 / height.max(1) as f32;
    let panel = panel_width as f32 / panel_height.max(1) as f32;
    if picture > panel {
        // wider than the panel, its sides cut
        let kept = panel / picture;
        [(1.0 - kept) / 2.0, 0.0, (1.0 + kept) / 2.0, 1.0]
    } else {
        let kept = picture / panel;
        [0.0, (1.0 - kept) / 2.0, 1.0, (1.0 + kept) / 2.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_border_covers_the_panel_cut_to_its_shape() {
        // a 16:9 picture on a 4:3 panel loses a quarter of its width, an
        // eighth each side
        let [left, top, right, bottom] = covering((1920, 1080), (640, 480));
        assert!((left - 0.125).abs() < 1e-4 && (right - 0.875).abs() < 1e-4 && top == 0.0 && bottom == 1.0);
        // a 4:3 picture on a 16:9 phone loses a quarter of its height
        let [left, top, right, bottom] = covering((640, 480), (1920, 1080));
        assert!(left == 0.0 && right == 1.0 && (top - 0.125).abs() < 1e-4 && (bottom - 0.875).abs() < 1e-4);
        assert_eq!(covering((640, 480), (640, 480)), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn the_stock_border_is_opaque_and_dark() {
        let pixels = stock_border(64, 48);
        assert_eq!(pixels.len(), 64 * 48 * 4);
        assert!(pixels.chunks(4).all(|pixel| pixel[3] == 255 && pixel[..3].iter().all(|&c| c < 64)));
    }

    #[test]
    fn a_png_border_loads_premultiplied() {
        let path = std::env::temp_dir().join(format!("pocket-native-border-{}.png", std::process::id()));
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 255, 255, 255, 255, 128]).unwrap();
        }
        let border = load_border(&path);
        let _ = std::fs::remove_file(&path);
        match border {
            Ok(Border::Picture { pixels, width, height }) => {
                assert_eq!((width, height), (2, 1));
                assert_eq!(pixels, vec![255, 0, 0, 255, 128, 128, 128, 128]);
            }
            _ => panic!("the border did not load"),
        }
    }
}

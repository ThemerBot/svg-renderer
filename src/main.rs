#[macro_use]
extern crate rocket;

use rocket::{
    config::LogLevel,
    http::{ContentType, Status},
    Config,
};
use std::fs::File;
use std::io::prelude::*;

type Rgb = [u8; 3];

fn parse_color(color: &str) -> Option<Rgb> {
    if color.len() != 6 || !color.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }

    let channel = |i: usize| u8::from_str_radix(&color[i..i + 2], 16).ok();

    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn is_light([red, green, blue]: Rgb) -> bool {
    let yiq = (u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114) / 1000;
    yiq >= 128
}

#[get("/render/<color1>/<color2>/<color3>/<color4>/<color5>")]
async fn render(
    color1: &str,
    color2: &str,
    color3: &str,
    color4: &str,
    color5: &str,
) -> (Status, (ContentType, Vec<u8>)) {
    let colors = [color1, color2, color3, color4, color5];

    let mut file = File::open("colors.svg").expect("Unable to open file");
    let mut svg_data = String::new();
    file.read_to_string(&mut svg_data)
        .expect("Unable to read file");

    for (index, color) in colors.iter().enumerate() {
        if let Some(rgb_color) = parse_color(color) {
            let text_color = if is_light(rgb_color) {
                "000000"
            } else {
                "FFFFFF"
            };

            svg_data = svg_data
                .replace(&format!("{{bg_{}}}", index).to_string(), color)
                .replace(&format!("{{text_{}}}", index).to_string(), text_color);
        } else {
            return (Status::BadRequest, (ContentType::HTML, vec![]));
        }
    }

    let rtree = usvg::Tree::from_str(&svg_data, &usvg::Options::default()).unwrap();

    let pixmap_size = rtree.svg_node().size.to_screen_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    resvg::render(&rtree, usvg::FitTo::Original, pixmap.as_mut()).unwrap();

    (Status::Ok, (ContentType::PNG, pixmap.encode_png().unwrap()))
}

#[rocket::main]
async fn main() {
    let mut config = Config::default();
    config.port = 9278;

    if !cfg!(debug_assertions) {
        config.log_level = LogLevel::Off;
    }

    rocket::custom(config)
        .mount("/", routes![render])
        .launch()
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_colors() {
        assert_eq!(parse_color("000000"), Some([0, 0, 0]));
        assert_eq!(parse_color("FFffFF"), Some([255, 255, 255]));
        assert_eq!(parse_color("1a2B3c"), Some([0x1a, 0x2b, 0x3c]));
    }

    #[test]
    fn rejects_invalid_colors() {
        for input in ["", "fff", "fffffff", "gggggg", "+f+f+f", "-f-f-f", " fffff", "aébcd"] {
            assert_eq!(parse_color(input), None, "{:?} should be rejected", input);
        }
    }

    #[test]
    fn picks_contrasting_text() {
        assert!(is_light([255, 255, 255]));
        assert!(is_light([255, 255, 0]));
        assert!(!is_light([0, 0, 0]));
        assert!(!is_light([0, 0, 255]));
    }
}

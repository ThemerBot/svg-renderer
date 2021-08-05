#[macro_use]
extern crate rocket;

use palette::Srgba;
use rocket::{
    config::LogLevel,
    http::{ContentType, Status},
    Config,
};
use std::fs::File;
use std::io::prelude::*;

type Color = Srgba<u64>;

fn parse_hex(hex: &str) -> Option<u64> {
    u64::from_str_radix(hex, 16).ok()
}

fn parse_color(color: &str) -> Option<Color> {
    if color.len() != 3 * 2 {
        return None;
    }

    let red = parse_hex(&color[0..2])?;
    let green = parse_hex(&color[2..4])?;
    let blue = parse_hex(&color[4..6])?;

    Some(Color::new(red, green, blue, 255))
}

fn is_light(color: Color) -> bool {
    let rgb = color.color;
    let yiq = (rgb.red * 299 + rgb.green * 587 + rgb.blue * 114) / 1000;
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

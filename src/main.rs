#[macro_use]
extern crate rocket;

use rocket::{
    config::LogLevel,
    http::{ContentType, Status},
    tokio::task::spawn_blocking,
    Config,
};

const TEMPLATE: &str = include_str!("../colors.svg");

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

/// Substitutes `{bg_N}` and `{text_N}` placeholders in a single pass over the
/// template. Fails with a user-facing message if any of the colors is invalid.
fn fill_template(colors: [&str; 5]) -> Result<String, String> {
    let mut text_colors = [""; 5];
    for (text_color, color) in text_colors.iter_mut().zip(colors) {
        let rgb = parse_color(color)
            .ok_or_else(|| format!("invalid color {:?}: expected 6 hex digits", color))?;

        *text_color = if is_light(rgb) { "000000" } else { "FFFFFF" };
    }

    let mut svg = String::with_capacity(TEMPLATE.len());
    let mut rest = TEMPLATE;

    while let Some(start) = rest.find('{') {
        svg.push_str(&rest[..start]);
        rest = &rest[start..];

        let replacement = rest.find('}').and_then(|end| {
            let value = placeholder_value(&rest[1..end], &colors, &text_colors)?;
            Some((value, end))
        });

        match replacement {
            Some((value, end)) => {
                svg.push_str(value);
                rest = &rest[end + 1..];
            }
            None => {
                svg.push('{');
                rest = &rest[1..];
            }
        }
    }

    svg.push_str(rest);
    Ok(svg)
}

fn placeholder_value<'a>(key: &str, bg: &[&'a str; 5], text: &[&'a str; 5]) -> Option<&'a str> {
    let (kind, index) = key.split_once('_')?;
    let index: usize = index.parse().ok()?;

    match kind {
        "bg" => bg.get(index).copied(),
        "text" => text.get(index).copied(),
        _ => None,
    }
}

#[get("/render/<color1>/<color2>/<color3>/<color4>/<color5>")]
async fn render(
    color1: &str,
    color2: &str,
    color3: &str,
    color4: &str,
    color5: &str,
) -> Result<(ContentType, Vec<u8>), (Status, String)> {
    let svg = fill_template([color1, color2, color3, color4, color5])
        .map_err(|message| (Status::BadRequest, message))?;

    let png = spawn_blocking(move || render_png(&svg))
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result)
        .map_err(|message| {
            error!("Failed to render image: {}", message);
            (Status::InternalServerError, "failed to render image".to_string())
        })?;

    Ok((ContentType::PNG, png))
}

fn render_png(svg: &str) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| format!("invalid SVG: {}", e))?;

    let size = tree.svg_node().size.to_screen_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or("could not allocate pixmap")?;
    resvg::render(&tree, usvg::FitTo::Original, pixmap.as_mut()).ok_or("could not render SVG")?;

    pixmap
        .encode_png()
        .map_err(|e| format!("could not encode PNG: {}", e))
}

#[rocket::main]
async fn main() {
    let mut config = Config::default();
    config.port = 9278;

    if !cfg!(debug_assertions) {
        config.log_level = LogLevel::Critical;
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

    #[test]
    fn fills_every_placeholder() {
        let svg = fill_template(["ffffff", "000000", "ff0000", "00ff00", "0000ff"]).unwrap();

        assert!(!svg.contains("{bg_"), "unfilled bg placeholder");
        assert!(!svg.contains("{text_"), "unfilled text placeholder");
        assert!(svg.contains(r##"fill="#ffffff" class="bg bg-1""##));
        assert!(svg.contains(r##"fill="#000000" d="M94.36"##));
        assert!(svg.contains(r##"fill="#000000" class="bg bg-2""##));
        assert!(svg.contains(r##"fill="#FFFFFF" d="M235.48"##));
    }

    #[test]
    fn rejects_template_with_invalid_color() {
        assert_eq!(
            fill_template(["ffffff", "000000", "nope!!", "00ff00", "0000ff"]),
            Err(r#"invalid color "nope!!": expected 6 hex digits"#.to_string())
        );
    }

    #[test]
    fn renders_png() {
        let svg = fill_template(["ffffff", "000000", "ff0000", "00ff00", "0000ff"]).unwrap();
        let png = render_png(&svg).unwrap();

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn leaves_unknown_braces_alone() {
        let bg = ["a"; 5];
        let text = ["b"; 5];
        assert_eq!(placeholder_value("bg_4", &bg, &text), Some("a"));
        assert_eq!(placeholder_value("text_0", &bg, &text), Some("b"));
        assert_eq!(placeholder_value("bg_5", &bg, &text), None);
        assert_eq!(placeholder_value("other_0", &bg, &text), None);
        assert_eq!(placeholder_value("", &bg, &text), None);
    }
}

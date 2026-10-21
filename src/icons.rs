use creamui_image::{ImageData, SvgSize};
use creamui_widgets::{IconImage, IconSource};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Icons {
    glyphs: HashMap<&'static str, IconSource>,
}

impl Icons {
    pub fn load() -> Self {
        let paths: &[(&'static str, &str)] = &[
            ("play", r#"<path d="M8 5.5v13l11-6.5Z"/>"#),
            ("pause", r#"<path d="M8 5v14M16 5v14" stroke-width="2.4"/>"#),
            ("next", r#"<path d="M6 5.5v13l9-6.5Z"/><path d="M17 5v14" stroke-width="2.2"/>"#),
            ("prev", r#"<path d="M18 5.5v13l-9-6.5Z"/><path d="M7 5v14" stroke-width="2.2"/>"#),
            (
                "shuffle",
                r#"<path d="M16 4h4v4M20 4l-7.5 7.5M16 20h4v-4M20 20l-7.5-7.5M4 8h3l2.5 2.5M4 16h3l2.5-2.5"/>"#,
            ),
            (
                "repeat",
                r#"<path d="M17 3l3 3-3 3M7 21l-3-3 3-3"/><path d="M20 6H8a4 4 0 0 0-4 4v2M4 18h12a4 4 0 0 0 4-4v-2"/>"#,
            ),
            (
                "repeat-one",
                r#"<path d="M17 3l3 3-3 3M7 21l-3-3 3-3"/><path d="M20 6H8a4 4 0 0 0-4 4v2M4 18h12a4 4 0 0 0 4-4v-2"/><path d="M11.5 9.5v5M11.5 9.5 10 11" stroke-width="1.3"/>"#,
            ),
            (
                "volume",
                r#"<path d="M4 9v6h4l6 4.5v-15L8 9H4Z"/><path d="M16.5 9.5a4 4 0 0 1 0 5M19 7a7.5 7.5 0 0 1 0 10"/>"#,
            ),
            (
                "mute",
                r#"<path d="M4 9v6h4l6 4.5v-15L8 9H4Z"/><path d="m17 9.5 5 5m0-5-5 5"/>"#,
            ),
            (
                "speed",
                r#"<circle cx="12" cy="13" r="8"/><path d="M12 13l3.5-3.5"/><path d="M12 5V3" stroke-width="1.2"/>"#,
            ),
            (
                "queue",
                r#"<path d="M4 6h16M4 11h16M4 16h9"/><circle cx="18.5" cy="17.5" r="2.6"/><path d="M18.5 15v-1.5" stroke-width="1.2"/>"#,
            ),
            (
                "open",
                r#"<path d="M3 8V6a2 2 0 0 1 2-2h4l3 3h7a2 2 0 0 1 2 2v2"/><path d="M3 8h18l-2 11a2 2 0 0 1-2 1.6H7A2 2 0 0 1 5 19L3 8Z"/>"#,
            ),
            ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
            ("close", r#"<path d="m6 6 12 12M6 18 18 6"/>"#),
            ("minimize", r#"<path d="M5 12h14"/>"#),
            ("maximize", r#"<rect x="5" y="5" width="14" height="14" rx="2"/>"#),
            (
                "music",
                r#"<path d="M9.5 18.5V6l11-2.2V17"/><ellipse cx="6.8" cy="18.5" rx="2.8" ry="2.5"/><ellipse cx="17.6" cy="17" rx="2.8" ry="2.5"/>"#,
            ),
            (
                "film",
                r#"<rect x="3" y="4.5" width="18" height="15" rx="2.5"/><path d="M8 4.5v15M16 4.5v15M3 9h5M3 15h5M16 9h5M16 15h5" stroke-width="1.3"/>"#,
            ),
            (
                "warning",
                r#"<path d="M12 4 2.5 20h19L12 4Z"/><path d="M12 10v4.5M12 17.4v.1" stroke-width="1.9"/>"#,
            ),
            (
                "app",
                r#"<circle cx="12" cy="12" r="9.5"/><path d="M9.7 7.8v8.4l7.2-4.2Z"/>"#,
            ),
            (
                "fullscreen",
                r#"<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>"#,
            ),
        ];
        let mut glyphs = HashMap::new();
        for (name, path) in paths {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{path}</svg>"##
            );
            glyphs.insert(*name, decode(&svg));
        }
        Self { glyphs }
    }

    pub fn get(&self, name: &str) -> IconSource {
        self.glyphs
            .get(name)
            .unwrap_or(&self.glyphs["music"])
            .clone()
    }
}

fn decode(svg: &str) -> IconSource {
    let image = ImageData::from_svg(svg.as_bytes(), SvgSize::Max(128)).expect("bundled SVG");
    IconSource::Image(IconImage {
        image: image.image().clone(),
        monochrome: true,
    })
}

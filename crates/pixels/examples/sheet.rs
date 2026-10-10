//! Draws a test sheet (every glyph, every shape) to a PPM image.
//! cargo run -p secretspace-pixels --example sheet -- out.ppm
use pixels::{Canvas, Rect, Rgba};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sheet.ppm".into());
    let mut c = Canvas::new(420, 240);
    c.clear(Rgba::rgb(7, 10, 18));
    let all: String = (32u8..127).map(|b| b as char).collect();
    for (i, chunk) in all.as_bytes().chunks(32).enumerate() {
        let line = std::str::from_utf8(chunk).unwrap_or("");
        c.text(8, 8 + i as i32 * 10, line, 1, Rgba::rgb(240, 240, 255));
    }
    c.text_shadowed(8, 44, "SECRETSPACE", 3, Rgba::rgb(87, 227, 137));
    c.text(
        8,
        72,
        "the quick brown fox jumps over 1,234 dogs",
        1,
        Rgba::rgb(255, 210, 150),
    );
    c.text(
        8,
        84,
        "José Zoë Łukasz Ñandú Åsa Ørjan straße ğüş - playing",
        1,
        Rgba::rgb(190, 220, 255),
    );
    for k in 0..8 {
        let x = 20.0 + k as f32 * 22.0;
        c.glow(
            x,
            110.0,
            14.0,
            Rgba::hsl(k as f32 * 45.0, 1.0, 0.6).fade(0.5),
        );
        c.circle(
            x,
            110.0,
            2.0 + k as f32,
            Rgba::hsl(k as f32 * 45.0, 0.9, 0.6),
        );
    }
    c.line(10.0, 140.0, 200.0, 170.0, 9.0, Rgba::hsl(200.0, 0.8, 0.5));
    c.ring(260.0, 120.0, 30.0, 3.0, Rgba::rgb(255, 60, 90));
    c.round_rect(
        Rect::new(220.0, 160.0, 160.0, 60.0),
        10.0,
        Rgba(255, 255, 255, 30),
    );
    c.round_rect_line(
        Rect::new(220.0, 160.0, 160.0, 60.0),
        10.0,
        2.0,
        Rgba::rgb(87, 227, 137),
    );
    c.text(232, 172, "PLAY", 4, Rgba::rgb(255, 255, 255));
    let mut out = format!("P6 {} {} 255\n", c.w, c.h).into_bytes();
    for px in c.data.chunks_exact(4) {
        out.extend_from_slice(&px[..3]);
    }
    std::fs::write(path, out).expect("write");
}

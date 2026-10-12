//! Иконка Forma: Holotype из рисунка `assets/logo/holotype.webp`: изометрическая стопка
//! страниц с закладками и графом связей и тремя кристаллами — знаком FORMA, как у Mycelium.
//!
//! Пока рисунок один на все размеры: и для экрана приветствия, и для панели задач.
//! Пустое поле вокруг рисунка обрезается, а уменьшение идёт по цвету, умноженному на
//! прозрачность, — иначе края темнеют от чёрных прозрачных пикселей.
//!
//! Файл подключает ещё build.rs, чтобы вшить иконку в .exe.

use std::sync::OnceLock;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba};

const LOGO: &[u8] = include_bytes!("../../../assets/logo/holotype.webp");

/// Поле вокруг рисунка — доля стороны квадрата.
const MARGIN: f32 = 0.03;

/// Рисунок, обрезанный до квадрата, с цветом, умноженным на прозрачность.
type Master = ImageBuffer<Rgba<f32>, Vec<f32>>;

fn master() -> &'static Master {
    static MASTER: OnceLock<Master> = OnceLock::new();
    MASTER.get_or_init(|| prepare(LOGO))
}

/// Раскодирует рисунок и вырезает квадрат вокруг непрозрачной части с небольшим полем.
fn prepare(bytes: &[u8]) -> Master {
    let img = image::load_from_memory(bytes).expect("рисунок иконки вшит в программу и раскодируется").to_rgba32f();
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 0.03 {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x0 > x1 {
        (x0, y0, x1, y1) = (0, 0, w - 1, h - 1);
    }
    let side = ((x1 - x0 + 1).max(y1 - y0 + 1) as f32 * (1.0 + 2.0 * MARGIN)).ceil() as u32;
    let left = (x0 + x1 + 1) as i64 / 2 - side as i64 / 2;
    let top = (y0 + y1 + 1) as i64 / 2 - side as i64 / 2;
    Master::from_fn(side, side, |x, y| {
        let (sx, sy) = (left + x as i64, top + y as i64);
        if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
            return Rgba([0.0; 4]);
        }
        let p = img.get_pixel(sx as u32, sy as u32);
        Rgba([p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
    })
}

/// Иконка `size`×`size` в RGBA (цвет не умножен на прозрачность).
pub fn rgba(size: u32) -> Vec<u8> {
    let scaled = image::imageops::resize(master(), size, size, FilterType::CatmullRom);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for p in scaled.pixels() {
        let a = p[3].clamp(0.0, 1.0);
        let color = |c: f32| if a > 0.0 { c / a } else { 0.0 };
        out.extend_from_slice(&[byte(color(p[0])), byte(color(p[1])), byte(color(p[2])), byte(a)]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_decodes_trimmed_and_square() {
        let m = master();
        assert_eq!(m.width(), m.height());
        // Поле обрезано: рисунок касается почти всех краёв квадрата.
        let px = rgba(64);
        assert_eq!(px.len(), 64 * 64 * 4);
        let opaque_rows = (0..64).filter(|y| (0..64).any(|x| px[((y * 64 + x) * 4 + 3) as usize] > 128)).count();
        assert!(opaque_rows >= 56, "рисунок занимает {opaque_rows} строк из 64");
        // Углы прозрачные.
        assert_eq!(px[3], 0);
        // ICON_PREVIEW=папка — сохранить картинки, чтобы посмотреть глазами.
        if let Some(dir) = std::env::var_os("ICON_PREVIEW") {
            for size in [16u32, 24, 32, 48, 256] {
                let path = std::path::Path::new(&dir).join(format!("icon-{size}.png"));
                image::save_buffer(path, &rgba(size), size, size, image::ColorType::Rgba8).unwrap();
            }
        }
    }
}

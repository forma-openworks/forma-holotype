//! Картинки мастера установки Forma: Holotype из логотипа `assets/logo/holotype.webp`.
//!
//! `holotype-installer-art <логотип> <папка>` пишет в папку:
//! - `small-<сторона>.png` — логотип на прозрачном для правого верхнего угла страниц;
//! - `large-light-<ширина>.png`, `large-dark-<ширина>.png` — полоса слева на первой и
//!   последней странице: мягкий градиент и свечение за логотипом, для светлой и тёмной темы.
//!
//! Размеры — те, что Inno Setup выбирает при масштабе от 100 до 250 %. Обрезка и
//! уменьшение — как у иконки (`crates/holotype/src/icon.rs`): по цвету, умноженному на
//! прозрачность, иначе края темнеют от чёрных прозрачных пикселей.

use std::io::Write;
use std::path::Path;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};

/// Рисунок с цветом, умноженным на прозрачность.
type Master = ImageBuffer<Rgba<f32>, Vec<f32>>;

/// Поле вокруг рисунка — доля стороны квадрата.
const MARGIN: f32 = 0.03;

/// Маленькая картинка: сторона квадрата при 100…250 %.
const SMALL: [u32; 7] = [58, 77, 97, 116, 124, 143, 159];

/// Большая картинка: ширина и высота при 100…250 % (пропорция 164:314).
const LARGE: [(u32, u32); 7] = [(202, 386), (269, 515), (336, 643), (403, 772), (430, 824), (498, 953), (534, 1022)];

/// Тема большой картинки: имя, цвет градиента сверху и снизу, цвет и сила свечения.
const THEMES: [(&str, &str, &str, &str, f32); 2] = [("light", "#f6f3ff", "#e6e0fa", "#c3b5ff", 0.45), ("dark", "#1f1c2c", "#121217", "#6f58f0", 0.30)];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, logo, out] = args.as_slice() else {
        eprintln!("usage: holotype-installer-art <logo.webp> <out dir>");
        std::process::exit(2);
    };
    let master = prepare(Path::new(logo));
    let out = Path::new(out);
    std::fs::create_dir_all(out).expect("папка для картинок");

    for side in SMALL {
        let logo = (side as f32 * 0.92).round() as u32;
        let img = compose(&master, side, side, logo, side as f32 / 2.0, side as f32 / 2.0, |_, _| [0.0; 4]);
        save(&img, &out.join(format!("small-{side}.png")));
    }

    for (name, top, bottom, glow, strength) in THEMES {
        let (top, bottom, glow) = (hex(top), hex(bottom), hex(glow));
        for (w, h) in LARGE {
            // Логотип чуть выше середины: под ним остаётся воздух, как на обложке.
            let (cx, cy) = (w as f32 / 2.0, h as f32 * 0.40);
            let radius = w as f32 * 0.80;
            let background = |x: f32, y: f32| {
                let base = mix(top, bottom, y / h as f32);
                let t = (1.0 - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / radius).clamp(0.0, 1.0);
                let c = mix(base, glow, strength * t * t * (3.0 - 2.0 * t));
                [c[0], c[1], c[2], 1.0]
            };
            let logo = (w as f32 * 0.70).round() as u32;
            let img = compose(&master, w, h, logo, cx, cy, background);
            save(&img, &out.join(format!("large-{name}-{w}.png")));
        }
    }
}

/// Пишет PNG, как libpng: данные (IDAT) кусками по 8 КБ. Inno Setup читает PNG декодером
/// Delphi, а тот отвергает некоторые файлы с одним большим IDAT («crc error»): так вышло с
/// IDAT длиной 0x20002, когда весь рисунок писался одним куском.
fn save(img: &RgbaImage, path: &Path) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), img.width(), img.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().expect("заголовок PNG");
    let mut stream = writer.stream_writer_with_size(8192).expect("поток PNG");
    stream.write_all(img.as_raw()).expect("данные PNG");
    stream.finish().expect("конец PNG");
}

/// Раскодирует логотип и вырезает квадрат вокруг непрозрачной части с небольшим полем.
fn prepare(path: &Path) -> Master {
    let img = image::open(path).unwrap_or_else(|e| panic!("логотип {}: {e}", path.display())).to_rgba32f();
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

/// Холст `w`×`h`: фон `background(x, y)` (RGBA без умножения) и поверх него логотип
/// стороной `logo` с центром в (`cx`, `cy`).
fn compose(master: &Master, w: u32, h: u32, logo: u32, cx: f32, cy: f32, background: impl Fn(f32, f32) -> [f32; 4]) -> RgbaImage {
    let scaled = image::imageops::resize(master, logo, logo, FilterType::CatmullRom);
    let (ox, oy) = ((cx - logo as f32 / 2.0).round() as i64, (cy - logo as f32 / 2.0).round() as i64);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    RgbaImage::from_fn(w, h, |x, y| {
        let b = background(x as f32 + 0.5, y as f32 + 0.5);
        let mut px = [b[0] * b[3], b[1] * b[3], b[2] * b[3], b[3]];
        let (lx, ly) = (x as i64 - ox, y as i64 - oy);
        if lx >= 0 && ly >= 0 && lx < logo as i64 && ly < logo as i64 {
            let p = scaled.get_pixel(lx as u32, ly as u32);
            let a = p[3].clamp(0.0, 1.0);
            for i in 0..3 {
                px[i] = p[i].max(0.0) + px[i] * (1.0 - a);
            }
            px[3] = a + px[3] * (1.0 - a);
        }
        if px[3] <= 0.0 {
            return Rgba([0, 0, 0, 0]);
        }
        Rgba([byte(px[0] / px[3]), byte(px[1] / px[3]), byte(px[2] / px[3]), byte(px[3])])
    })
}

/// `#rrggbb` → RGB от 0 до 1.
fn hex(s: &str) -> [f32; 3] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).expect("цвет #rrggbb");
    [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

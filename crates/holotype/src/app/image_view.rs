//! Картинка во вкладке, как в Obsidian, а управление — как в редакторе изображений Blender:
//! колесо приближает к указателю, перетаскивание двигает, двойной щелчок — «вписать» или
//! 100 %. Под прозрачными местами — шахматка, внизу — размер, формат, вес и масштаб.
//!
//! Читает картинки загрузчик `forma-images` (все форматы); огромные он уменьшает под предел
//! видеокарты, а масштаб здесь считается от исходного размера.

use std::path::Path;

use eframe::egui::{self, Align, Color32, Id, Layout, Rect, RichText, Sense, TextureHandle, TextureOptions, Ui, UiBuilder, Vec2, pos2, vec2};
use forma_i18n::t;
use forma_ui::theme::Palette;

/// Масштаб и место: `zoom` — пикселей экрана на пиксель картинки (`None` — вписать в
/// область), `center` — точка картинки (в её пикселях) в середине области.
#[derive(Clone, Copy)]
struct View {
    zoom: Option<f32>,
    center: Vec2,
}

const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 64.0;
/// Высота строки сведений внизу.
const BAR: f32 = 28.0;
/// Сторона клетки шахматки (в точках).
const CELL: f32 = 8.0;

/// Рисует картинку `abs` (полный путь) во вкладке `tab`. Не прочиталась — возвращает почему.
pub fn show(ui: &mut Ui, fpal: &Palette, tab: u64, abs: &Path) -> Option<String> {
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let rect = Rect::from_min_max(full.min, pos2(full.right(), full.bottom() - BAR));
    let bar = Rect::from_min_max(pos2(full.left(), rect.bottom()), full.max);
    let uri = holotype_view::plugin_ui::file_url(abs);
    let ppp = ui.ctx().pixels_per_point();
    let image = egui::Image::new(uri);
    let texture = match image.load_for_size(ui.ctx(), rect.size()) {
        Ok(egui::load::TexturePoll::Ready { texture }) => texture,
        Ok(egui::load::TexturePoll::Pending { .. }) => {
            ui.put(Rect::from_center_size(rect.center(), vec2(24.0, 24.0)), egui::Spinner::new().size(20.0).color(fpal.accent));
            return None;
        }
        Err(e) => return Some(e.to_string()),
    };
    let info = forma_images::info(abs);
    // Исходный размер: картинку могли уменьшить под предел видеокарты.
    let size = info.as_ref().map_or(texture.size, |i| vec2(i.width as f32, i.height as f32)).max(Vec2::splat(1.0));
    // Своё место и масштаб у каждой картинки вкладки: открыли другую — она снова вписана.
    let id = Id::new(("holotype-image", tab, abs));
    let mut view: View = ui.data(|d| d.get_temp(id)).unwrap_or(View { zoom: None, center: size / 2.0 });
    // Вписать: целиком в область, но не крупнее 100 %.
    let fit = (rect.width() * ppp / size.x).min(rect.height() * ppp / size.y).min(1.0);
    let mut zoom = view.zoom.unwrap_or(fit);

    let resp = ui.interact(rect, id.with("area"), Sense::click_and_drag());
    if resp.hovered()
        && let Some(p) = resp.hover_pos()
    {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (scroll * 0.004).exp() * pinch;
        if (factor - 1.0).abs() > 1e-4 {
            // Точка картинки под указателем остаётся под ним.
            let at = view.center + (p - rect.center()) * ppp / zoom;
            zoom = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            view.center = at - (p - rect.center()) * ppp / zoom;
            view.zoom = Some(zoom);
        }
    }
    if resp.dragged() {
        view.center -= resp.drag_delta() * ppp / zoom;
        view.zoom = Some(zoom);
    }
    if resp.double_clicked() {
        if view.zoom.is_some_and(|z| (z - 1.0).abs() < 1e-3) {
            view.zoom = None;
        } else {
            if let Some(p) = resp.interact_pointer_pos() {
                view.center += (p - rect.center()) * ppp / zoom;
            }
            view.zoom = Some(1.0);
        }
    }
    if view.zoom.is_none() {
        view.center = size / 2.0;
    }
    zoom = view.zoom.unwrap_or(fit);
    view.center = view.center.clamp(Vec2::ZERO, size);

    let image_rect = Rect::from_min_size(rect.center() - view.center * zoom / ppp, size * zoom / ppp);
    let painter = ui.painter_at(rect);
    checker(ui, &painter, image_rect, fpal.dark);
    // Близко — пиксели как есть, как в Blender; издалека — сглаживание.
    let options = if zoom >= 2.0 { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
    let mut clipped = ui.new_child(UiBuilder::new().max_rect(rect));
    clipped.set_clip_rect(rect.intersect(ui.clip_rect()));
    image.texture_options(options).paint_at(&clipped, image_rect);

    // Строка сведений: размер, формат, вес, масштаб и кнопки «Вписать» и «100 %».
    ui.painter().rect_filled(bar, 0.0, fpal.bg_header);
    ui.painter().hline(bar.x_range(), bar.top(), egui::Stroke::new(1.0, fpal.border));
    let mut row = ui.new_child(UiBuilder::new().max_rect(bar.shrink2(vec2(10.0, 3.0))).layout(Layout::left_to_right(Align::Center)));
    let mut parts = vec![format!("{} × {}", size.x as u32, size.y as u32)];
    if let Some(i) = &info {
        parts.push(i.format.clone());
        parts.push(forma_plugins::human_size(i.bytes, &t!("unit-mb"), &t!("unit-kb")));
    }
    parts.push(format!("{:.0} %", zoom * 100.0));
    row.label(RichText::new(parts.join("  ·  ")).size(12.0).color(fpal.text_muted));
    row.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.add_enabled(view.zoom != Some(1.0), egui::Button::new(RichText::new("100 %").size(12.0))).on_hover_text(t!("image-actual-size")).clicked() {
            view.zoom = Some(1.0);
        }
        if ui.add_enabled(view.zoom.is_some(), egui::Button::new(RichText::new(t!("image-fit")).size(12.0))).clicked() {
            view.zoom = None;
        }
    });
    ui.data_mut(|d| d.insert_temp(id, view));
    None
}

/// Шахматка под картинкой (видно прозрачные места): одна текстура 2×2, повторённая на всю
/// картинку и сдвинутая вместе с ней.
fn checker(ui: &Ui, painter: &egui::Painter, image_rect: Rect, dark: bool) {
    let visible = image_rect.intersect(painter.clip_rect());
    if !visible.is_positive() {
        return;
    }
    let id = Id::new(("holotype-image-checker", dark));
    let texture: TextureHandle = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| {
        let (a, b) = if dark { (Color32::from_gray(0x2b), Color32::from_gray(0x36)) } else { (Color32::from_gray(0xff), Color32::from_gray(0xe4)) };
        let image = egui::ColorImage::new([2, 2], vec![a, b, b, a]);
        let options = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..TextureOptions::NEAREST };
        let t = ui.ctx().load_texture("holotype-image-checker", image, options);
        ui.data_mut(|d| d.insert_temp(id, t.clone()));
        t
    });
    let uv = |p: egui::Pos2| ((p - image_rect.min) / (2.0 * CELL)).to_pos2();
    painter.image(texture.id(), visible, Rect::from_min_max(uv(visible.min), uv(visible.max)), Color32::WHITE);
}

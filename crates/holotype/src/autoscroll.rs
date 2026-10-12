//! Прокрутка «как в браузере»: нажать колёсико мыши и вести курсор вверх или вниз.
//!
//! Нажала и повела — страница едет, пока колёсико зажато. Щёлкнула и отпустила — режим
//! остаётся включённым до следующего щелчка или Esc. Скорость растёт с расстоянием от
//! точки нажатия, а в самой точке рисуется значок со стрелками.

use eframe::egui::{self, CursorIcon, Id, Key, LayerId, Order, PointerButton, Pos2, Shape, Stroke, Ui, vec2};

use crate::theme::Palette;

/// Мёртвая зона вокруг точки нажатия: пока курсор в ней, страница стоит.
const DEAD: f32 = 10.0;
/// Сдвиг курсора, после которого отпускание колёсика выключает прокрутку.
const DRAG: f32 = 6.0;

#[derive(Clone, Copy)]
struct State {
    anchor: Pos2,
    /// Какая область прокрутки едет.
    area: Id,
    /// Режим «щёлкнула и отпустила»: держится до следующего щелчка.
    sticky: bool,
    moved: bool,
    /// Номер кадра, в котором состояние обновлялось (устаревшее — сбрасываем).
    pass: u64,
}

fn key() -> Id {
    Id::new("autoscroll")
}

/// Вызывать в самом конце содержимого вертикальной `ScrollArea`: тогда прокрутка
/// достаётся именно ей, а не вложенным горизонтальным областям (блоки кода, диаграммы).
pub fn area(ui: &Ui, area: Id, pal: &Palette) {
    let ctx = ui.ctx().clone();
    let pass = ctx.cumulative_pass_nr();
    let mut state: Option<State> = ctx.data(|d| d.get_temp(key())).filter(|s: &State| s.pass + 2 >= pass);
    let (mid_pressed, mid_released, other_pressed, esc, pos, dt) = ctx.input(|i| {
        (
            i.pointer.button_pressed(PointerButton::Middle),
            i.pointer.button_released(PointerButton::Middle),
            i.pointer.button_pressed(PointerButton::Primary) || i.pointer.button_pressed(PointerButton::Secondary),
            i.key_pressed(Key::Escape),
            i.pointer.hover_pos(),
            i.stable_dt.min(0.05),
        )
    });

    match state {
        None => {
            // Щелчок колёсиком по ссылке открывает её в новой вкладке — там не прокручиваем.
            let over_link = ctx.output(|o| o.cursor_icon == CursorIcon::PointingHand);
            match pos {
                Some(p) if mid_pressed && ui.clip_rect().contains(p) && !over_link => {
                    state = Some(State { anchor: p, area, sticky: false, moved: false, pass });
                }
                _ => return,
            }
        }
        Some(s) if s.area != area => return,
        Some(_) => {}
    }
    let Some(mut s) = state else { return };

    // Любой другой щелчок, повторный щелчок колёсиком или Esc выключают режим.
    if esc || other_pressed || (s.sticky && mid_pressed) {
        ctx.data_mut(|d| d.remove::<State>(key()));
        ctx.request_repaint();
        return;
    }
    let p = pos.unwrap_or(s.anchor);
    if (p - s.anchor).length() > DRAG {
        s.moved = true;
    }
    if mid_released && !s.sticky {
        if s.moved {
            ctx.data_mut(|d| d.remove::<State>(key()));
            ctx.request_repaint();
            return;
        }
        s.sticky = true;
    }
    s.pass = pass;
    ctx.data_mut(|d| d.insert_temp(key(), s));

    let dy = p.y - s.anchor.y;
    let excess = (dy.abs() - DEAD).max(0.0);
    // Точек в секунду: медленно возле точки нажатия и всё быстрее при удалении.
    let speed = excess * 8.0 + excess * excess * 0.035;
    if speed > 0.0 {
        ui.scroll_with_delta_animation(vec2(0.0, -dy.signum() * speed * dt), egui::style::ScrollAnimation::none());
    }
    ctx.request_repaint();
    ctx.set_cursor_icon(if excess == 0.0 {
        CursorIcon::AllScroll
    } else if dy < 0.0 {
        CursorIcon::ResizeNorth
    } else {
        CursorIcon::ResizeSouth
    });
    paint_anchor(&ctx, s.anchor, dy, pal);
}

/// Значок в точке нажатия: кружок с двумя стрелками, активная стрелка подсвечена.
fn paint_anchor(ctx: &egui::Context, c: Pos2, dy: f32, pal: &Palette) {
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("autoscroll-anchor")));
    painter.circle(c, 15.0, pal.bg_elevated.gamma_multiply(0.94), Stroke::new(1.2, pal.border));
    painter.circle_filled(c, 2.4, pal.text_muted);
    for (dir, on) in [(-1.0f32, dy < -DEAD), (1.0, dy > DEAD)] {
        let tip = c + vec2(0.0, dir * 10.5);
        let base = c + vec2(0.0, dir * 5.0);
        let color = if on { pal.accent } else { pal.text_muted };
        // Обход по часовой стрелке для обеих стрелок — так egui ровно сглаживает края.
        let (a, b) = (base + vec2(4.6 * -dir, 0.0), base - vec2(4.6 * -dir, 0.0));
        painter.add(Shape::convex_polygon(vec![tip, a, b], color, Stroke::NONE));
    }
}

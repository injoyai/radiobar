use tauri::{Position, Size, WebviewWindow};

use crate::snap::{expanded_rect, is_visible_on, snap_bar, Rect, PANEL_H};

pub const BAR_W: i32 = 340;
pub const BAR_H: i32 = 64;

pub fn work_rects(window: &WebviewWindow) -> Vec<Rect> {
    let mut rects = Vec::new();
    if let Ok(monitors) = window.available_monitors() {
        for monitor in monitors {
            let scale = monitor.scale_factor();
            let pos = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            rects.push(Rect {
                x: pos.x as i32,
                y: pos.y as i32,
                w: size.width as i32,
                h: size.height as i32,
            });
        }
    }
    rects
}

pub fn current_work_rect(window: &WebviewWindow) -> Option<Rect> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let pos = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    Some(Rect {
        x: pos.x as i32,
        y: pos.y as i32,
        w: size.width as i32,
        h: size.height as i32,
    })
}

pub fn current_bar_rect(window: &WebviewWindow) -> Option<Rect> {
    let scale = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.outer_size().ok()?.to_logical::<f64>(scale);
    Some(Rect {
        x: pos.x.round() as i32,
        y: pos.y.round() as i32,
        w: size.width.round() as i32,
        h: size.height.round() as i32,
    })
}

pub fn apply_position(window: &WebviewWindow, x: i32, y: i32) {
    let _ = window.set_position(Position::Logical(tauri::LogicalPosition::new(
        x as f64, y as f64,
    )));
}

pub fn apply_size(window: &WebviewWindow, w: i32, h: i32) {
    let _ = window.set_size(Size::Logical(tauri::LogicalSize::new(
        w as f64, h as f64,
    )));
}

/// 启动时决定折叠条位置：记忆位置在任一屏幕可见则沿用，否则主屏右上角。
pub fn initial_bar_rect(window: &WebviewWindow, saved: Option<(i32, i32)>) -> Rect {
    let works = work_rects(window);
    if let Some((x, y)) = saved {
        let rect = Rect {
            x,
            y,
            w: BAR_W,
            h: BAR_H,
        };
        if is_visible_on(rect, &works) {
            return rect;
        }
    }
    let work = works
        .first()
        .copied()
        .unwrap_or(Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        });
    Rect {
        x: work.x + work.w - BAR_W - 24,
        y: work.y + 24,
        w: BAR_W,
        h: BAR_H,
    }
}

/// 折叠条移动后贴边；返回新的 canonical 折叠条位置。
pub fn snap_bar_window(window: &WebviewWindow) -> Option<Rect> {
    let bar = current_bar_rect(window)?;
    let work = current_work_rect(window)?;
    let snapped = snap_bar(bar, work);
    if snapped.x != bar.x || snapped.y != bar.y {
        apply_position(window, snapped.x, snapped.y);
    }
    Some(snapped)
}

/// 展开 / 收起面板；返回 canonical 折叠条位置。
pub fn apply_panel(window: &WebviewWindow, bar: Rect, open: bool) -> Rect {
    if open {
        if let Some(work) = current_work_rect(window) {
            let expanded = expanded_rect(
                Rect {
                    w: BAR_W,
                    h: BAR_H,
                    ..bar
                },
                work,
                PANEL_H,
            );
            apply_size(window, BAR_W, expanded.h);
            apply_position(window, expanded.x, expanded.y);
        } else {
            apply_size(window, BAR_W, PANEL_H);
        }
    } else {
        apply_size(window, BAR_W, BAR_H);
        apply_position(window, bar.x, bar.y);
    }
    bar
}

/// 面板展开时窗口被拖动：由窗口矩形反推折叠条矩形。
pub fn bar_rect_while_open(window: &WebviewWindow, previous: Rect) -> Option<Rect> {
    let rect = current_bar_rect(window)?;
    let grew_up = previous.y + BAR_H <= rect.y + rect.h && previous.y != rect.y;
    let y = if grew_up {
        rect.y + rect.h - BAR_H
    } else {
        rect.y
    };
    Some(Rect {
        x: rect.x,
        y,
        w: BAR_W,
        h: BAR_H,
    })
}

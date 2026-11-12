use tauri::{Position, Size, WebviewWindow};

use crate::snap::{expanded_rect, is_visible_on, snap_bar, Rect, PANEL_H};

pub const BAR_W: i32 = 340;
pub const BAR_H: i32 = 64;

/// 显示器工作区（排除任务栏）转成逻辑坐标的 Rect。
fn monitor_work_rect(monitor: &tauri::Monitor) -> Rect {
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    Rect {
        x: (area.position.x as f64 / scale).round() as i32,
        y: (area.position.y as f64 / scale).round() as i32,
        w: (area.size.width as f64 / scale).round() as i32,
        h: (area.size.height as f64 / scale).round() as i32,
    }
}

pub fn work_rects(window: &WebviewWindow) -> Vec<Rect> {
    let mut rects = Vec::new();
    if let Ok(monitors) = window.available_monitors() {
        for monitor in monitors {
            rects.push(monitor_work_rect(&monitor));
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
    Some(monitor_work_rect(&monitor))
}

/// 无边框窗口在 Windows 上带 DWM 阴影：外框比客户区每边大 8px（宽 +16、高 +8）。
/// 折叠条的几何一律用客户区（= WebView 视口）坐标，写回窗口时再换算成外框。
fn shadow_delta(window: &WebviewWindow) -> (i32, i32, i32, i32) {
    let Ok(scale) = window.scale_factor() else {
        return (0, 0, 0, 0);
    };
    let (Ok(outer_pos), Ok(inner_pos), Ok(outer_size), Ok(inner_size)) = (
        window.outer_position(),
        window.inner_position(),
        window.outer_size(),
        window.inner_size(),
    ) else {
        return (0, 0, 0, 0);
    };
    let dx = ((outer_pos.x - inner_pos.x) as f64 / scale).round() as i32;
    let dy = ((outer_pos.y - inner_pos.y) as f64 / scale).round() as i32;
    let dw = ((outer_size.width as f64 - inner_size.width as f64) / scale).round() as i32;
    let dh = ((outer_size.height as f64 - inner_size.height as f64) / scale).round() as i32;
    (dx, dy, dw, dh)
}

pub fn current_bar_rect(window: &WebviewWindow) -> Option<Rect> {
    let scale = window.scale_factor().ok()?;
    let pos = window.inner_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(Rect {
        x: pos.x.round() as i32,
        y: pos.y.round() as i32,
        w: size.width.round() as i32,
        h: size.height.round() as i32,
    })
}

pub fn apply_position(window: &WebviewWindow, x: i32, y: i32) {
    // set_position 收的是外框位置；client.x = outer.x - dx（dx = outer - inner，通常 -8）。
    let (dx, dy, _, _) = shadow_delta(window);
    let _ = window.set_position(Position::Logical(tauri::LogicalPosition::new(
        (x + dx) as f64,
        (y + dy) as f64,
    )));
}

pub fn apply_size(window: &WebviewWindow, w: i32, h: i32) {
    // Tauri 的 set_size 收的是客户区尺寸（tauri.conf 的 width/height 同理），无需阴影补偿。
    let _ = window.set_size(Size::Logical(tauri::LogicalSize::new(
        w as f64, h as f64,
    )));
}

/// 启动时决定折叠条位置：记忆位置在任一屏幕可见则沿用，否则主屏右下角（留 24px 边距）。
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
        y: work.y + work.h - BAR_H - 24,
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

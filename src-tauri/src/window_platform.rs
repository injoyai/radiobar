use tauri::{Position, Size, WebviewWindow};

use crate::snap::{expanded_rect, is_visible_on, snap_bar, Rect, PANEL_H};

pub const BAR_W: i32 = 340;
pub const BAR_H: i32 = 64;

/// 逻辑内容四周留出的透明边，用来让 CSS 画圆角投影
/// （DWM 的矩形阴影在圆角处会露出直角，所以窗口不再要 DWM 阴影）。
pub const SHADOW_PAD: i32 = 16;

/// 逻辑尺寸 → 外框尺寸（四周各加 SHADOW_PAD）。
pub fn outer_size(w: i32, h: i32) -> (i32, i32) {
    (w + SHADOW_PAD * 2, h + SHADOW_PAD * 2)
}

/// 逻辑位置 → 外框位置（左上各减 SHADOW_PAD）。
pub fn outer_position(x: i32, y: i32) -> (i32, i32) {
    (x - SHADOW_PAD, y - SHADOW_PAD)
}

/// 外框矩形 → 逻辑矩形（去掉四周留边）。
pub fn inner_rect(x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
    (
        x + SHADOW_PAD,
        y + SHADOW_PAD,
        w - SHADOW_PAD * 2,
        h - SHADOW_PAD * 2,
    )
}

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

/// 折叠条的几何一律用**逻辑坐标**（= 去掉留边后的内容矩形），
/// 写回窗口时再换算成外框；留边是我们自己定的常量，不必运行时测量。
fn shadow_delta() -> (i32, i32, i32, i32) {
    (-SHADOW_PAD, -SHADOW_PAD, SHADOW_PAD * 2, SHADOW_PAD * 2)
}

/// 窗口外框（含留边）换算成逻辑内容矩形。
pub fn current_bar_rect(window: &WebviewWindow) -> Option<Rect> {
    let scale = window.scale_factor().ok()?;
    let pos = window.inner_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    let (x, y, w, h) = inner_rect(
        pos.x.round() as i32,
        pos.y.round() as i32,
        size.width.round() as i32,
        size.height.round() as i32,
    );
    Some(Rect { x, y, w, h })
}

pub fn apply_position(window: &WebviewWindow, x: i32, y: i32) {
    // set_position 收的是外框位置；外框左上 = 逻辑左上 - 留边。
    let (dx, dy, _, _) = shadow_delta();
    let _ = window.set_position(Position::Logical(tauri::LogicalPosition::new(
        (x + dx) as f64,
        (y + dy) as f64,
    )));
}

pub fn apply_size(window: &WebviewWindow, w: i32, h: i32) {
    // set_size 收的是内容尺寸，须把四周留边加回去（CSS 只画 w×h 的那块）。
    let (ow, oh) = outer_size(w, h);
    let _ = window.set_size(Size::Logical(tauri::LogicalSize::new(
        ow as f64, oh as f64,
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

/// 展开 / 收起面板；返回 canonical 折叠条位置与本次展开方向（朝上为 true）。
/// 方向判断与 `snap::expanded_rect` 同源，避免前后端各算一套。
pub fn apply_panel(window: &WebviewWindow, bar: Rect, open: bool) -> (Rect, bool) {
    if open {
        let work = current_work_rect(window);
        let grows_up = match work {
            Some(work) => bar.y + BAR_H / 2 >= work.y + work.h / 2,
            None => false,
        };
        if let Some(work) = work {
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
        (bar, grows_up)
    } else {
        apply_size(window, BAR_W, BAR_H);
        apply_position(window, bar.x, bar.y);
        (bar, false)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 逻辑尺寸换算成外框尺寸时，四周各加一圈留边。
    #[test]
    fn test_outer_size_adds_padding_on_all_sides() {
        assert_eq!(outer_size(BAR_W, BAR_H), (372, 96));
        assert_eq!(outer_size(BAR_W, PANEL_H), (372, 472));
    }

    /// 逻辑位置换算成外框位置时，左上各减一圈留边。
    #[test]
    fn test_outer_position_subtracts_padding() {
        assert_eq!(outer_position(100, 200), (84, 184));
        assert_eq!(outer_position(0, 0), (-16, -16));
    }

    /// 外框矩形换回逻辑矩形后，应与最初传入的逻辑值一致（往返无损）。
    #[test]
    fn test_inner_rect_roundtrips_with_outer_helpers() {
        let (ox, oy) = outer_position(1234, 952);
        let (ow, oh) = outer_size(BAR_W, BAR_H);
        assert_eq!(
            inner_rect(ox, oy, ow, oh),
            (1234, 952, BAR_W, BAR_H)
        );
    }

    /// 副屏负坐标同样要正确（漏减留边会让窗口整体偏移）。
    #[test]
    fn test_outer_helpers_handle_negative_coordinates() {
        let (ox, oy) = outer_position(-1920, -40);
        assert_eq!((ox, oy), (-1936, -56));
        let (ow, oh) = outer_size(BAR_W, BAR_H);
        assert_eq!(inner_rect(ox, oy, ow, oh), (-1920, -40, BAR_W, BAR_H));
    }
}

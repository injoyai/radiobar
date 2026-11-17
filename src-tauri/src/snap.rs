#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 吸附距离。必须大于 `window_platform::SHADOW_PAD`（16）：鼠标拖动时系统把
/// 窗口外框顶边钳在屏幕顶，逻辑 y 最低只到 SHADOW_PAD，阈值若比它小则上边永远吸不上。
pub const SNAP_THRESHOLD: i32 = 20;
pub const PANEL_H: i32 = 440;

/// 折叠条贴边：与 work 四边距离 < SNAP_THRESHOLD 时贴齐，返回新 Rect。
pub fn snap_bar(bar: Rect, work: Rect) -> Rect {
    let mut x = bar.x;
    let left_dist = (bar.x - work.x).abs();
    let right_dist = (work.x + work.w - (bar.x + bar.w)).abs();
    if left_dist < SNAP_THRESHOLD {
        x = work.x;
    } else if right_dist < SNAP_THRESHOLD {
        x = work.x + work.w - bar.w;
        if x < work.x {
            x = work.x;
        }
    }

    let mut y = bar.y;
    let top_dist = (bar.y - work.y).abs();
    let bottom_dist = (work.y + work.h - (bar.y + bar.h)).abs();
    if top_dist < SNAP_THRESHOLD {
        y = work.y;
    } else if bottom_dist < SNAP_THRESHOLD {
        y = work.y + work.h - bar.h;
        if y < work.y {
            y = work.y;
        }
    }

    Rect { x, y, ..bar }
}

/// 面板展开位置：默认向下；bar 中心在工作区下半部时向上；结果整窗 clamp 进 work。
pub fn expanded_rect(bar: Rect, work: Rect, panel_h: i32) -> Rect {
    let h = panel_h.min(work.h);
    let bar_center = bar.y + bar.h / 2;
    let work_center = work.y + work.h / 2;
    let mut y = if bar_center >= work_center {
        bar.y + bar.h - h
    } else {
        bar.y
    };
    if y + h > work.y + work.h {
        y = work.y + work.h - h;
    }
    if y < work.y {
        y = work.y;
    }
    Rect { x: bar.x, y, w: bar.w, h }
}

/// 位置记忆校验：bar 是否与任一显示器工作区有交集。
pub fn is_visible_on(rect: Rect, work_areas: &[Rect]) -> bool {
    work_areas.iter().any(|work| {
        let left = rect.x.max(work.x);
        let top = rect.y.max(work.y);
        let right = (rect.x + rect.w).min(work.x + work.w);
        let bottom = (rect.y + rect.h).min(work.y + work.h);
        right > left && bottom > top
    })
}

/// 吸附缓动：ease-out quadratic，返回第 step 帧（共 steps 帧）相对起点的位移。
/// 前段快、后段慢，松手后让条「滑」向边缘而不是整窗瞬移 16px。
pub fn eased_offset(delta: (i32, i32), step: u32, steps: u32) -> (i32, i32) {
    let t = (step as f64 / steps as f64).clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - t) * (1.0 - t);
    (
        (delta.0 as f64 * e).round() as i32,
        (delta.1 as f64 * e).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };

    fn bar(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            w: 340,
            h: 64,
        }
    }

    #[test]
    fn test_snap_each_edge() {
        assert_eq!(snap_bar(bar(5, 500), WORK).x, 0);
        assert_eq!(snap_bar(bar(1920 - 340 - 5, 500), WORK).x, 1920 - 340);
        assert_eq!(snap_bar(bar(500, 5), WORK).y, 0);
        assert_eq!(snap_bar(bar(500, 1080 - 64 - 5), WORK).y, 1080 - 64);
    }

    /// 拖动吸顶后落盘的坐标：外框顶边被钳在 y=0，逻辑 y 只剩 SHADOW_PAD(16)，
    /// 仍须吸附到 0，否则可见条与屏幕上边缘之间留下 16px 空隙。
    #[test]
    fn test_snap_top_from_drag_clamped_position() {
        let b = bar(500, 16);
        assert_eq!(snap_bar(b, WORK).y, 0);
    }

    #[test]
    fn test_no_snap_beyond_threshold() {
        let b = bar(20, 200);
        assert_eq!(snap_bar(b, WORK), b);
    }

    /// 展开态回归：拖动吸顶后逻辑 y 停在 16，必须先 snap 再算展开矩形，
    /// 否则展开后条顶距屏幕顶端仍差 16px。
    #[test]
    fn test_expanded_after_snap_is_flush_with_top() {
        let snapped = snap_bar(bar(500, 16), WORK);
        assert_eq!(snapped.y, 0);
        // 贴顶时面板向下长，条顶就是展开矩形的顶。
        let out = expanded_rect(snapped, WORK, PANEL_H);
        assert_eq!(out.y, 0);
        assert_eq!(out.h, PANEL_H);
        assert_eq!(out.x, snapped.x);
    }

    /// 反向对照：不先 snap 直接展开，缝隙 16px 原样保留——这就是被修掉的旧行为。
    #[test]
    fn test_expanded_without_snap_keeps_gap() {
        let out = expanded_rect(bar(500, 16), WORK, PANEL_H);
        assert_eq!(out.y, 16);
    }

    /// 缓动必须单调、起于 0、终于完整位移，且首帧不是整段跳变。
    #[test]
    fn test_eased_offset_is_monotonic_and_bounded() {
        let delta = (0, -16);
        let steps = 7;
        assert_eq!(eased_offset(delta, 0, steps), (0, 0));
        assert_eq!(eased_offset(delta, steps, steps), delta);
        let first = eased_offset(delta, 1, steps).1;
        assert!(first < 0 && first > delta.1);
        let mut prev = 0;
        for step in 1..=steps {
            let y = eased_offset(delta, step, steps).1;
            assert!(y <= prev, "step {step}: {y} > {prev}");
            prev = y;
        }
        assert_eq!(prev, delta.1);
    }

    #[test]
    fn test_negative_coordinate_monitor() {
        let work = Rect {
            x: -1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let snapped = snap_bar(bar(-1920 + 1920 - 340 - 5, 500), work);
        assert_eq!(snapped.x, -340);
        let snapped_left = snap_bar(bar(-1920 + 5, 500), work);
        assert_eq!(snapped_left.x, -1920);
    }

    #[test]
    fn test_expanded_grows_down_normally() {
        let out = expanded_rect(bar(0, 100), WORK, PANEL_H);
        assert_eq!(out.y, 100);
        assert_eq!(out.h, PANEL_H);
        assert_eq!(out.w, 340);
    }

    #[test]
    fn test_expanded_grows_up_near_bottom() {
        let b = bar(0, 1080 - 64);
        let out = expanded_rect(b, WORK, PANEL_H);
        assert_eq!(out.y, 1080 - PANEL_H);
        assert_eq!(out.h, PANEL_H);
        assert!(out.y >= WORK.y);
    }

    #[test]
    fn test_expanded_clamped_in_short_work_area() {
        let work = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 300,
        };
        let out = expanded_rect(bar(0, 100), work, PANEL_H);
        assert_eq!(out.y, 0);
        assert_eq!(out.h, 300);
        assert!(out.y + out.h <= work.y + work.h);
    }

    #[test]
    fn test_is_visible_on() {
        let next_to = Rect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let partially = bar(1900, 100);
        assert!(is_visible_on(partially, &[WORK, next_to]));
        let offscreen = bar(4000, 100);
        assert!(!is_visible_on(offscreen, &[WORK, next_to]));
    }
}

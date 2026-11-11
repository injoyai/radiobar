#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub const SNAP_THRESHOLD: i32 = 12;
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

    #[test]
    fn test_no_snap_beyond_threshold() {
        let b = bar(20, 200);
        assert_eq!(snap_bar(b, WORK), b);
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

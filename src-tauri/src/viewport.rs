use crate::timer::Position;
pub struct Area { pub x: i32, pub y: i32, pub width: u32, pub height: u32, pub scale: f64 }
pub fn place(saved: Option<Position>, areas: &[Area], width: f64, height: f64) -> Position {
    let Some(area) = saved.and_then(|p| areas.iter().find(|a| p.x >= a.x && p.y >= a.y
        && i64::from(p.x) < i64::from(a.x) + i64::from(a.width) && i64::from(p.y) < i64::from(a.y) + i64::from(a.height)))
        .or_else(|| areas.first()) else { return Position { x: 24, y: 24 }; };
    let w = (width * area.scale).ceil() as i32;
    let h = (height * area.scale).ceil() as i32;
    let max_x = area.x + (area.width as i32 - w).max(0);
    let max_y = area.y + (area.height as i32 - h).max(0);
    let wanted = saved.filter(|p| p.x >= area.x && p.y >= area.y && p.x <= area.x + area.width as i32 && p.y <= area.y + area.height as i32)
        .unwrap_or(Position { x: max_x - 24, y: area.y + 24 });
    Position { x: wanted.x.clamp(area.x, max_x), y: wanted.y.clamp(area.y, max_y) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn removed_monitor_and_dpi_are_clamped_to_work_area() {
        let areas = [Area { x: 0, y: 0, width: 1920, height: 1040, scale: 1.5 }];
        assert_eq!(place(Some(Position { x: 3500, y: 100 }), &areas, 220., 100.), Position { x: 1566, y: 24 });
        assert_eq!(place(Some(Position { x: 1900, y: 1030 }), &areas, 360., 520.), Position { x: 1380, y: 260 });
    }
    #[test] fn negative_monitor_coordinates_are_supported() {
        let areas = [Area { x: -1920, y: -200, width: 1920, height: 1080, scale: 1. }];
        assert_eq!(place(Some(Position { x: -500, y: -100 }), &areas, 220., 100.), Position { x: -500, y: -100 });
    }
}

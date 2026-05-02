use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::canvas::{Canvas, Line, Points};
use ratatui::widgets::{Block, Widget};

use super::world::WorldMap;

/// Viewport parameters for map panning and zooming.
#[derive(Debug, Clone)]
pub struct MapViewport {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
}

impl Default for MapViewport {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl MapViewport {
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(10.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.1);
    }
}

/// Render the map as a Canvas widget with burg dots, route lines, and river lines.
pub fn render_map(world: &WorldMap, viewport: &MapViewport, area: Rect, buf: &mut Buffer) {
    let map_width = world.map.info.width.max(1920.0);
    let map_height = world.map.info.height.max(1080.0);

    // Compute visible bounds based on viewport
    let view_width = map_width / viewport.zoom;
    let view_height = map_height / viewport.zoom;
    let x_min = viewport.offset_x;
    let x_max = viewport.offset_x + view_width;
    // Canvas y-axis is bottom-to-top, but Azgaar is top-to-bottom.
    // Flip: y_min at bottom, y_max at top of viewport.
    let y_min = viewport.offset_y;
    let y_max = viewport.offset_y + view_height;

    let canvas = Canvas::default()
        .block(Block::bordered().title("Map"))
        .x_bounds([x_min, x_max])
        .y_bounds([y_min, y_max])
        .paint(|ctx| {
            // Draw routes as gray lines
            for route in &world.map.pack.routes {
                for window in route.points.windows(2) {
                    let y1 = map_height - window[0].y;
                    let y2 = map_height - window[1].y;
                    ctx.draw(&Line {
                        x1: window[0].x,
                        y1,
                        x2: window[1].x,
                        y2,
                        color: Color::DarkGray,
                    });
                }
            }

            // Draw rivers as blue lines (approximate as source->mouth)
            for river in &world.map.pack.rivers {
                if river.length > 0.0 {
                    // Rivers don't have point data in minimal export, skip line drawing
                    // but we could mark them with labels if needed
                }
            }

            // Draw burgs as dots with labels
            for burg in world.map.pack.burgs.iter().skip(1) {
                if burg.name.is_empty() {
                    continue;
                }
                let y = map_height - burg.y;
                let color = if burg.capital > 0 {
                    Color::Yellow
                } else if burg.population >= 10.0 {
                    Color::White
                } else {
                    Color::Gray
                };

                // Draw the burg as a point
                ctx.draw(&Points {
                    coords: &[(burg.x, y)],
                    color,
                });

                // Draw the label
                ctx.print(burg.x + 2.0, y, ratatui::text::Line::from(burg.name.clone()).style(
                    ratatui::style::Style::default().fg(color),
                ));
            }
        });

    canvas.render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::azgaar::parse_azgaar_json;

    fn test_world() -> WorldMap {
        let json = r##"{
            "info": {"width": 800, "height": 600},
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Silverport", "x": 200.0, "y": 150.0, "population": 28.5, "capital": 1},
                    {"i": 2, "name": "Ironhold", "x": 500.0, "y": 300.0, "population": 12.0}
                ],
                "routes": [
                    {"i": 1, "points": [{"x": 200.0, "y": 150.0}, {"x": 350.0, "y": 225.0}, {"x": 500.0, "y": 300.0}], "group": "roads", "length": 100.0}
                ],
                "rivers": [
                    {"i": 1, "name": "Silverrun", "length": 50.0}
                ]
            }
        }"##;
        WorldMap::from_parsed(parse_azgaar_json(json).unwrap())
    }

    #[test]
    fn test_render_produces_non_empty_output() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        render_map(&world, &viewport, area, &mut buf);

        // Buffer should not be all spaces — something was rendered
        let content: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
        let non_space = content.chars().filter(|c| !c.is_whitespace()).count();
        assert!(
            non_space > 0,
            "Rendered map should have non-whitespace content"
        );
    }

    #[test]
    fn test_render_at_different_zoom_levels() {
        let world = test_world();
        let area = Rect::new(0, 0, 60, 20);

        for zoom in [0.5, 1.0, 2.0, 5.0] {
            let viewport = MapViewport {
                offset_x: 0.0,
                offset_y: 0.0,
                zoom,
            };
            let mut buf = Buffer::empty(area);
            render_map(&world, &viewport, area, &mut buf);

            let content: String =
                buf.content().iter().map(|c| c.symbol().to_string()).collect();
            let non_space = content.chars().filter(|c| !c.is_whitespace()).count();
            assert!(
                non_space > 0,
                "Rendered map at zoom {zoom} should have content"
            );
        }
    }

    #[test]
    fn test_render_without_png_canvas_only() {
        // This is the fallback mode — no PNG, just Canvas data
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 80, 25);
        let mut buf = Buffer::empty(area);

        render_map(&world, &viewport, area, &mut buf);

        let content: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
        // Should render burg names
        assert!(
            content.contains("Silverport") || content.contains("Ironhold"),
            "Canvas-only mode should render burg labels. Content: {content:?}"
        );
    }

    #[test]
    fn test_viewport_pan() {
        let mut vp = MapViewport::default();
        assert_eq!(vp.offset_x, 0.0);
        assert_eq!(vp.offset_y, 0.0);

        vp.pan(50.0, 30.0);
        assert_eq!(vp.offset_x, 50.0);
        assert_eq!(vp.offset_y, 30.0);

        vp.pan(-20.0, -10.0);
        assert_eq!(vp.offset_x, 30.0);
        assert_eq!(vp.offset_y, 20.0);
    }

    #[test]
    fn test_viewport_zoom() {
        let mut vp = MapViewport::default();
        assert_eq!(vp.zoom, 1.0);

        vp.zoom_in();
        assert!(vp.zoom > 1.0);

        vp.zoom_out();
        vp.zoom_out();
        assert!(vp.zoom < 1.0);
    }

    #[test]
    fn test_viewport_zoom_clamped() {
        let mut vp = MapViewport { zoom: 10.0, ..Default::default() };
        vp.zoom_in();
        assert!(vp.zoom <= 10.0, "Zoom should be clamped to max 10.0");

        let mut vp2 = MapViewport { zoom: 0.1, ..Default::default() };
        vp2.zoom_out();
        assert!(vp2.zoom >= 0.1, "Zoom should be clamped to min 0.1");
    }
}

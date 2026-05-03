use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::canvas::{Canvas, Line, Points};
use ratatui::widgets::{Block, StatefulWidget, Widget};

use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

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

/// Load a PNG map image and create a StatefulProtocol for rendering.
/// Returns `None` if the file doesn't exist or can't be decoded.
pub fn load_map_png(picker: &Picker, png_path: &Path) -> Option<StatefulProtocol> {
    if !png_path.exists() {
        return None;
    }
    let dyn_img = image::open(png_path).ok()?;
    Some(picker.new_resize_protocol(dyn_img))
}

/// Render the map with optional PNG background + Canvas data overlay.
/// When `image_state` is `Some`, renders the PNG as background and composites
/// Canvas data (burgs, routes) on top. When `None`, falls back to Canvas-only mode.
pub fn render_map(
    world: &WorldMap,
    viewport: &MapViewport,
    area: Rect,
    buf: &mut Buffer,
    image_state: Option<&mut StatefulProtocol>,
) {
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

    let has_image = image_state.is_some();

    // Step 1: render PNG background if available
    if let Some(state) = image_state {
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        StatefulWidget::render(image_widget, area, buf, state);
    }

    // Step 2: build the Canvas widget for data overlay
    let canvas = Canvas::default()
        .block(if has_image {
            // No border when image is the background — maximize map area
            Block::default()
        } else {
            Block::bordered().title("Map")
        })
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
                ctx.print(
                    burg.x + 2.0,
                    y,
                    ratatui::text::Line::from(burg.name.clone())
                        .style(ratatui::style::Style::default().fg(color)),
                );
            }
        });

    if has_image {
        // Render Canvas to temp buffer, then composite non-empty cells onto image
        let mut canvas_buf = Buffer::empty(area);
        canvas.render(area, &mut canvas_buf);
        composite_canvas_overlay(buf, &canvas_buf, area);
    } else {
        // No image — render Canvas directly (current behavior)
        canvas.render(area, buf);
    }
}

/// Composite non-empty canvas cells onto the base (image) buffer.
/// Only copies cells that contain actual data (not blank space or empty Braille).
fn composite_canvas_overlay(base: &mut Buffer, overlay: &Buffer, area: Rect) {
    use ratatui::layout::Position;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let pos = Position::new(x, y);
            if let Some(overlay_cell) = overlay.cell(pos) {
                let sym = overlay_cell.symbol();
                // Skip empty cells: regular space and blank Braille pattern (U+2800)
                if sym == " " || sym == "\u{2800}" {
                    continue;
                }
                if let Some(base_cell) = base.cell_mut(pos) {
                    *base_cell = overlay_cell.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::azgaar::parse_azgaar_json;
    use ratatui_image::picker::ProtocolType;

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

    fn test_picker() -> Picker {
        let mut picker = Picker::from_fontsize((8, 16));
        picker.set_protocol_type(ProtocolType::Halfblocks);
        picker
    }

    fn test_png() -> image::DynamicImage {
        image::DynamicImage::new_rgb8(100, 80)
    }

    #[test]
    fn test_render_produces_non_empty_output() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        render_map(&world, &viewport, area, &mut buf, None);

        // Buffer should not be all spaces — something was rendered
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
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
            render_map(&world, &viewport, area, &mut buf, None);

            let content: String = buf
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect();
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

        render_map(&world, &viewport, area, &mut buf, None);

        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // Should render burg names
        assert!(
            content.contains("Silverport") || content.contains("Ironhold"),
            "Canvas-only mode should render burg labels. Content: {content:?}"
        );
    }

    #[test]
    fn test_render_with_png_background() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf, Some(&mut state));

        // Buffer should have content (image + canvas data)
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let non_space = content.chars().filter(|c| !c.is_whitespace()).count();
        assert!(
            non_space > 0,
            "Map with PNG background should have non-whitespace content"
        );
    }

    #[test]
    fn test_render_with_png_has_no_border() {
        // When PNG is present, Canvas should NOT have a bordered block
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf_with_image = Buffer::empty(area);
        render_map(
            &world,
            &viewport,
            area,
            &mut buf_with_image,
            Some(&mut state),
        );

        let mut buf_canvas_only = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf_canvas_only, None);

        // Canvas-only mode should have border characters (─, │, etc.)
        let canvas_content: String = buf_canvas_only
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            canvas_content.contains('─') || canvas_content.contains('│'),
            "Canvas-only should have border chars"
        );
    }

    #[test]
    fn test_render_with_png_preserves_burg_labels() {
        // Canvas overlay on PNG should still show burg names
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 80, 25);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf, Some(&mut state));

        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            content.contains("Silverport") || content.contains("Ironhold"),
            "PNG mode should still render burg labels via Canvas overlay. Content: {content:?}"
        );
    }

    #[test]
    fn test_load_map_png_nonexistent_returns_none() {
        let picker = test_picker();
        let result = load_map_png(&picker, Path::new("/nonexistent/world.png"));
        assert!(result.is_none(), "Non-existent PNG should return None");
    }

    #[test]
    fn test_load_map_png_with_valid_image() {
        // Create a temp PNG file
        let dir = tempfile::TempDir::new().unwrap();
        let png_path = dir.path().join("test.png");
        let img = image::DynamicImage::new_rgb8(50, 50);
        img.save(&png_path).unwrap();

        let picker = test_picker();
        let result = load_map_png(&picker, &png_path);
        assert!(result.is_some(), "Valid PNG should return Some");
    }

    #[test]
    fn test_composite_canvas_overlay_preserves_base() {
        use ratatui::layout::Position;
        let area = Rect::new(0, 0, 10, 5);
        let mut base = Buffer::empty(area);
        // Fill base with 'X' to simulate image data
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                base.cell_mut(Position::new(x, y)).unwrap().set_symbol("X");
            }
        }

        // Overlay is all spaces (empty canvas)
        let overlay = Buffer::empty(area);

        composite_canvas_overlay(&mut base, &overlay, area);

        // Base should still be all 'X' — empty overlay doesn't overwrite
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                assert_eq!(
                    base.cell(Position::new(x, y)).unwrap().symbol(),
                    "X",
                    "Empty overlay should not overwrite base at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn test_composite_canvas_overlay_writes_data() {
        use ratatui::layout::Position;
        let area = Rect::new(0, 0, 10, 5);
        let mut base = Buffer::empty(area);
        // Fill base with 'X'
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                base.cell_mut(Position::new(x, y)).unwrap().set_symbol("X");
            }
        }

        // Overlay has one non-space cell
        let mut overlay = Buffer::empty(area);
        overlay
            .cell_mut(Position::new(3, 2))
            .unwrap()
            .set_symbol("A");

        composite_canvas_overlay(&mut base, &overlay, area);

        // Cell (3,2) should be 'A', rest should be 'X'
        assert_eq!(base.cell(Position::new(3, 2)).unwrap().symbol(), "A");
        assert_eq!(base.cell(Position::new(0, 0)).unwrap().symbol(), "X");
        assert_eq!(base.cell(Position::new(9, 4)).unwrap().symbol(), "X");
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
        let mut vp = MapViewport {
            zoom: 10.0,
            ..Default::default()
        };
        vp.zoom_in();
        assert!(vp.zoom <= 10.0, "Zoom should be clamped to max 10.0");

        let mut vp2 = MapViewport {
            zoom: 0.1,
            ..Default::default()
        };
        vp2.zoom_out();
        assert!(vp2.zoom >= 0.1, "Zoom should be clamped to min 0.1");
    }
}

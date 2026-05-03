use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, StatefulWidget, Widget, Wrap};

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

/// Build metadata lines from the WorldMap for display alongside the image.
fn build_metadata_lines(world: &WorldMap) -> Vec<Line<'static>> {
    let pack = &world.map.pack;
    let info = &world.map.info;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Map name / title
    let name = if info.map_name.is_empty() {
        "World Map".to_string()
    } else {
        info.map_name.clone()
    };
    lines.push(Line::from(Span::styled(
        name,
        Style::default().fg(Color::Cyan).bold(),
    )));
    lines.push(Line::from(""));

    // Counts
    let burg_count = pack
        .burgs
        .iter()
        .skip(1)
        .filter(|b| !b.name.is_empty())
        .count();
    let state_count = pack
        .states
        .iter()
        .skip(1)
        .filter(|s| !s.name.is_empty())
        .count();
    let culture_count = pack
        .cultures
        .iter()
        .skip(1)
        .filter(|c| !c.name.is_empty())
        .count();
    let religion_count = pack
        .religions
        .iter()
        .skip(1)
        .filter(|r| !r.name.is_empty())
        .count();
    let river_count = pack.rivers.iter().filter(|r| !r.name.is_empty()).count();
    let route_count = pack.routes.len();

    lines.push(Line::from(vec![
        Span::styled("Burgs: ", Style::default().fg(Color::Yellow)),
        Span::raw(burg_count.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("States: ", Style::default().fg(Color::Yellow)),
        Span::raw(state_count.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Cultures: ", Style::default().fg(Color::Yellow)),
        Span::raw(culture_count.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Religions: ", Style::default().fg(Color::Yellow)),
        Span::raw(religion_count.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Rivers: ", Style::default().fg(Color::Yellow)),
        Span::raw(river_count.to_string()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Routes: ", Style::default().fg(Color::Yellow)),
        Span::raw(route_count.to_string()),
    ]));
    lines.push(Line::from(""));

    // States list
    if state_count > 0 {
        lines.push(Line::from(Span::styled(
            "States",
            Style::default().fg(Color::Cyan).bold(),
        )));
        for state in pack.states.iter().skip(1) {
            if state.name.is_empty() {
                continue;
            }
            let label = if state.form.is_empty() {
                state.name.clone()
            } else {
                format!("{} ({})", state.name, state.form)
            };
            lines.push(Line::from(format!("  {label}")));
        }
        lines.push(Line::from(""));
    }

    // Capitals
    let capitals: Vec<&crate::map::azgaar::Burg> = pack
        .burgs
        .iter()
        .skip(1)
        .filter(|b| b.capital > 0 && !b.name.is_empty())
        .collect();
    if !capitals.is_empty() {
        lines.push(Line::from(Span::styled(
            "Capitals",
            Style::default().fg(Color::Cyan).bold(),
        )));
        for burg in &capitals {
            let pop = (burg.population * 1000.0) as u64;
            lines.push(Line::from(format!("  {} (pop: ~{})", burg.name, pop)));
        }
        lines.push(Line::from(""));
    }

    // Cultures list
    if culture_count > 0 {
        lines.push(Line::from(Span::styled(
            "Cultures",
            Style::default().fg(Color::Cyan).bold(),
        )));
        for culture in pack.cultures.iter().skip(1) {
            if culture.name.is_empty() {
                continue;
            }
            lines.push(Line::from(format!("  {}", culture.name)));
        }
        lines.push(Line::from(""));
    }

    // Hint
    lines.push(Line::from(Span::styled(
        "Esc: exit | +/-: zoom | arrows: pan",
        Style::default().fg(Color::DarkGray),
    )));

    lines
}

/// Render the map: PNG image + metadata sidebar.
/// When `image_state` is `Some`, renders the PNG image.
/// When `None`, shows metadata only with a message about the missing image.
pub fn render_map(
    world: &WorldMap,
    viewport: &MapViewport,
    area: Rect,
    buf: &mut Buffer,
    image_state: Option<&mut StatefulProtocol>,
) {
    // Suppress unused variable warnings — viewport is used for image cropping in app.rs
    let _ = viewport;

    let metadata_lines = build_metadata_lines(world);

    if let Some(state) = image_state {
        // Split: image (most of the space) + metadata sidebar (right)
        let sidebar_width = 30u16.min(area.width / 3);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(sidebar_width)])
            .split(area);

        // Render PNG image
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        StatefulWidget::render(image_widget, chunks[0], buf, state);

        // Render metadata sidebar
        let sidebar = Paragraph::new(metadata_lines)
            .block(Block::default().borders(Borders::LEFT))
            .wrap(Wrap { trim: false });
        sidebar.render(chunks[1], buf);
    } else {
        // No image — show metadata only with a hint
        let mut lines = vec![
            Line::from(Span::styled(
                "No map image found (map/world.png)",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
        ];
        lines.extend(metadata_lines);

        let paragraph = Paragraph::new(lines)
            .block(Block::bordered().title("Map"))
            .wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::azgaar::parse_azgaar_json;
    use ratatui_image::picker::ProtocolType;

    fn test_world() -> WorldMap {
        let json = r##"{
            "info": {"width": 800, "height": 600, "mapName": "Test World"},
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Silverport", "x": 200.0, "y": 150.0, "population": 28.5, "capital": 1},
                    {"i": 2, "name": "Ironhold", "x": 500.0, "y": 300.0, "population": 12.0}
                ],
                "states": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Kingdom of Light", "formName": "Kingdom"}
                ],
                "cultures": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Elven"}
                ],
                "religions": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Order of Light", "type": "Organized"}
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

    fn buffer_content(buf: &Buffer) -> String {
        buf.content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn test_render_without_png_shows_metadata() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        render_map(&world, &viewport, area, &mut buf, None);

        let content = buffer_content(&buf);
        assert!(content.contains("Burgs"), "Should show burg count");
        assert!(content.contains("States"), "Should show state count");
        assert!(
            content.contains("No map image"),
            "Should indicate missing PNG"
        );
    }

    #[test]
    fn test_render_without_png_shows_map_name() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);

        render_map(&world, &viewport, area, &mut buf, None);

        let content = buffer_content(&buf);
        assert!(content.contains("Test World"), "Should show map name");
    }

    #[test]
    fn test_render_with_png_shows_image_and_sidebar() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 80, 25);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf, Some(&mut state));

        let content = buffer_content(&buf);
        // Sidebar should have metadata
        assert!(content.contains("Burgs"), "Sidebar should show metadata");
        // Buffer should have non-whitespace content (image + sidebar)
        let non_space = content.chars().filter(|c| !c.is_whitespace()).count();
        assert!(non_space > 0, "Should have content");
    }

    #[test]
    fn test_render_with_png_sidebar_shows_states() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 80, 25);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf, Some(&mut state));

        let content = buffer_content(&buf);
        assert!(
            content.contains("Kingdom of Light"),
            "Sidebar should list states"
        );
    }

    #[test]
    fn test_render_with_png_sidebar_shows_capitals() {
        let world = test_world();
        let viewport = MapViewport::default();
        let area = Rect::new(0, 0, 80, 30);
        let picker = test_picker();
        let dyn_img = test_png();
        let mut state = picker.new_resize_protocol(dyn_img);

        let mut buf = Buffer::empty(area);
        render_map(&world, &viewport, area, &mut buf, Some(&mut state));

        let content = buffer_content(&buf);
        assert!(
            content.contains("Silverport"),
            "Sidebar should show capital"
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
        let dir = tempfile::TempDir::new().unwrap();
        let png_path = dir.path().join("test.png");
        let img = image::DynamicImage::new_rgb8(50, 50);
        img.save(&png_path).unwrap();

        let picker = test_picker();
        let result = load_map_png(&picker, &png_path);
        assert!(result.is_some(), "Valid PNG should return Some");
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

    #[test]
    fn test_build_metadata_lines_counts() {
        let world = test_world();
        let lines = build_metadata_lines(&world);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(text.contains('2'), "Should show burg count of 2");
        assert!(text.contains('1'), "Should show state count of 1");
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

            let content = buffer_content(&buf);
            let non_space = content.chars().filter(|c| !c.is_whitespace()).count();
            assert!(
                non_space > 0,
                "Rendered map at zoom {zoom} should have content"
            );
        }
    }
}

use serde::Deserialize;

/// Top-level Azgaar's Fantasy Map Generator Minimal JSON export.
#[derive(Debug, Deserialize)]
pub struct AzgaarMap {
    #[serde(default)]
    pub info: MapInfo,
    pub pack: Pack,
}

/// Map metadata.
#[derive(Debug, Default, Deserialize)]
pub struct MapInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "mapName")]
    pub map_name: String,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub seed: String,
}

/// The pack section containing all map entities.
#[derive(Debug, Default, Deserialize)]
pub struct Pack {
    #[serde(default)]
    pub burgs: Vec<Burg>,
    #[serde(default)]
    pub states: Vec<State>,
    #[serde(default)]
    pub provinces: Vec<Province>,
    #[serde(default)]
    pub cultures: Vec<Culture>,
    #[serde(default)]
    pub religions: Vec<Religion>,
    #[serde(default)]
    pub rivers: Vec<River>,
    #[serde(default)]
    pub routes: Vec<Route>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub zones: Vec<Zone>,
}

/// A settlement (city, town, village).
#[derive(Debug, Clone, Deserialize)]
pub struct Burg {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub population: f64,
    #[serde(default)]
    pub culture: u32,
    #[serde(default)]
    pub state: u32,
    #[serde(rename = "type", default)]
    pub burg_type: String,
    #[serde(default)]
    pub capital: u8,
    #[serde(default)]
    pub port: u8,
    #[serde(default)]
    pub citadel: u8,
    #[serde(default)]
    pub walls: u8,
}

/// A political entity (kingdom, republic, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct State {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub capital: u32,
    #[serde(default)]
    pub area: f64,
    #[serde(default)]
    pub rural: f64,
    #[serde(default)]
    pub urban: f64,
    #[serde(default)]
    pub neighbors: Vec<u32>,
}

/// A province within a state.
#[derive(Debug, Clone, Deserialize)]
pub struct Province {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: u32,
    #[serde(default)]
    pub capital: u32,
    #[serde(default)]
    pub area: f64,
}

/// A culture group.
#[derive(Debug, Clone, Deserialize)]
pub struct Culture {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub culture_type: String,
    #[serde(default)]
    pub color: String,
}

/// A religion.
#[derive(Debug, Clone, Deserialize)]
pub struct Religion {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub religion_type: String,
    #[serde(default)]
    pub deity: String,
}

/// A river.
#[derive(Debug, Clone, Deserialize)]
pub struct River {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source: u32,
    #[serde(default)]
    pub mouth: u32,
    #[serde(default)]
    pub discharge: f64,
    #[serde(default)]
    pub length: f64,
}

/// A route (road, trail, or sea route).
#[derive(Debug, Clone, Deserialize)]
pub struct Route {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub points: Vec<RoutePoint>,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub length: f64,
}

/// A single point along a route.
#[derive(Debug, Clone, Deserialize)]
pub struct RoutePoint {
    pub x: f64,
    pub y: f64,
}

/// A map marker (custom POI).
#[derive(Debug, Clone, Deserialize)]
pub struct Marker {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(rename = "type", default)]
    pub marker_type: String,
}

/// A named zone (danger zone, territory, etc.).
#[derive(Debug, Clone, Deserialize)]
pub struct Zone {
    #[serde(default)]
    pub i: u32,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub zone_type: String,
    #[serde(default)]
    pub color: String,
}

/// Parse an Azgaar Minimal JSON export from a string.
pub fn parse_azgaar_json(json: &str) -> Result<AzgaarMap, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to parse Azgaar JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_azgaar_json() -> &'static str {
        r##"{
            "info": {
                "version": "1.0",
                "mapName": "Test World",
                "width": 1920,
                "height": 1080,
                "seed": "12345"
            },
            "pack": {
                "burgs": [
                    {"i": 0, "name": "", "x": 0, "y": 0},
                    {"i": 1, "name": "Silverport", "x": 512.3, "y": 380.1, "population": 28.5, "culture": 1, "state": 1, "type": "City", "capital": 1, "port": 1, "citadel": 1, "walls": 1},
                    {"i": 2, "name": "Ironhold", "x": 800.0, "y": 500.0, "population": 12.0, "culture": 1, "state": 1, "type": "Town", "capital": 0, "port": 0}
                ],
                "states": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Kingdom of Light", "form": "Monarchy", "color": "#4a90d9", "capital": 1, "area": 45000, "rural": 120, "urban": 85, "neighbors": [2]}
                ],
                "cultures": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Elven", "type": "Lake", "color": "#8fc77e"}
                ],
                "religions": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Order of Light", "type": "Organized", "deity": "Sol"}
                ],
                "rivers": [
                    {"i": 1, "name": "Silverrun", "source": 500, "mouth": 100, "discharge": 45.2, "length": 120.5}
                ],
                "routes": [
                    {"i": 1, "points": [{"x": 512.3, "y": 380.1}, {"x": 650.0, "y": 440.0}, {"x": 800.0, "y": 500.0}], "group": "roads", "length": 55.3}
                ],
                "markers": [
                    {"i": 1, "icon": "sword", "x": 300, "y": 400, "type": "battlesite"}
                ],
                "zones": [
                    {"i": 1, "name": "Danger Zone", "type": "danger", "color": "#ff0000"}
                ]
            }
        }"##
    }

    #[test]
    fn test_parse_minimal_azgaar_json() {
        let map = parse_azgaar_json(minimal_azgaar_json()).unwrap();

        assert_eq!(map.info.map_name, "Test World");
        assert_eq!(map.info.width, 1920.0);
        assert_eq!(map.info.height, 1080.0);

        // burgs[0] is sentinel, burgs[1] is Silverport
        assert_eq!(map.pack.burgs.len(), 3);
        assert_eq!(map.pack.burgs[1].name, "Silverport");
        assert_eq!(map.pack.burgs[1].population, 28.5);
        assert_eq!(map.pack.burgs[1].capital, 1);
        assert_eq!(map.pack.burgs[1].port, 1);
        assert_eq!(map.pack.burgs[1].burg_type, "City");
        assert_eq!(map.pack.burgs[2].name, "Ironhold");

        assert_eq!(map.pack.states.len(), 2);
        assert_eq!(map.pack.states[1].name, "Kingdom of Light");
        assert_eq!(map.pack.states[1].form, "Monarchy");

        assert_eq!(map.pack.cultures.len(), 2);
        assert_eq!(map.pack.cultures[1].name, "Elven");

        assert_eq!(map.pack.religions.len(), 2);
        assert_eq!(map.pack.religions[1].deity, "Sol");

        assert_eq!(map.pack.rivers.len(), 1);
        assert_eq!(map.pack.rivers[0].name, "Silverrun");
        assert_eq!(map.pack.rivers[0].length, 120.5);

        assert_eq!(map.pack.routes.len(), 1);
        assert_eq!(map.pack.routes[0].points.len(), 3);
        assert_eq!(map.pack.routes[0].group, "roads");

        assert_eq!(map.pack.markers.len(), 1);
        assert_eq!(map.pack.markers[0].icon, "sword");

        assert_eq!(map.pack.zones.len(), 1);
        assert_eq!(map.pack.zones[0].name, "Danger Zone");
    }

    #[test]
    fn test_parse_missing_optional_sections() {
        let json = r##"{
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Lone Town", "x": 100, "y": 200}
                ]
            }
        }"##;

        let map = parse_azgaar_json(json).unwrap();
        assert_eq!(map.info.map_name, ""); // default
        assert_eq!(map.pack.burgs.len(), 2);
        assert_eq!(map.pack.burgs[1].name, "Lone Town");
        assert!(map.pack.states.is_empty());
        assert!(map.pack.cultures.is_empty());
        assert!(map.pack.religions.is_empty());
        assert!(map.pack.rivers.is_empty());
        assert!(map.pack.routes.is_empty());
        assert!(map.pack.markers.is_empty());
        assert!(map.pack.zones.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_azgaar_json("not json at all");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_empty_pack() {
        let json = r##"{"pack": {}}"##;
        let map = parse_azgaar_json(json).unwrap();
        assert!(map.pack.burgs.is_empty());
        assert!(map.pack.states.is_empty());
    }

    #[test]
    fn test_burg_default_fields() {
        let json = r##"{"pack": {"burgs": [{"i": 1, "name": "Bare"}]}}"##;
        let map = parse_azgaar_json(json).unwrap();
        let burg = &map.pack.burgs[0];
        assert_eq!(burg.name, "Bare");
        assert_eq!(burg.x, 0.0);
        assert_eq!(burg.y, 0.0);
        assert_eq!(burg.population, 0.0);
        assert_eq!(burg.capital, 0);
        assert_eq!(burg.port, 0);
        assert_eq!(burg.burg_type, "");
    }

    #[test]
    fn test_province_parsing() {
        let json = r##"{"pack": {"provinces": [{"i": 0, "name": ""}, {"i": 1, "name": "Northern March", "state": 1, "capital": 5, "area": 12000}]}}"##;
        let map = parse_azgaar_json(json).unwrap();
        assert_eq!(map.pack.provinces.len(), 2);
        assert_eq!(map.pack.provinces[1].name, "Northern March");
        assert_eq!(map.pack.provinces[1].area, 12000.0);
    }
}

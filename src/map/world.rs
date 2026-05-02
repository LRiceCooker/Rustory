use std::path::Path;

use super::azgaar::{self, AzgaarMap, Burg, Route, State};

/// WorldMap wraps parsed Azgaar data and provides query methods.
#[derive(Debug)]
pub struct WorldMap {
    pub map: AzgaarMap,
}

impl WorldMap {
    /// Load a WorldMap from a JSON file path.
    pub fn load(path: &Path) -> Result<Self, String> {
        let json =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read map file: {e}"))?;
        let map = azgaar::parse_azgaar_json(&json)?;
        Ok(Self { map })
    }

    /// Create a WorldMap from an already-parsed AzgaarMap.
    pub fn from_parsed(map: AzgaarMap) -> Self {
        Self { map }
    }

    /// Get a burg by exact name (case-insensitive). Skips sentinel at index 0.
    pub fn get_burg(&self, name: &str) -> Option<&Burg> {
        let name_lower = name.to_lowercase();
        self.map
            .pack
            .burgs
            .iter()
            .skip(1)
            .find(|b| b.name.to_lowercase() == name_lower)
    }

    /// Fuzzy search burgs by name (case-insensitive substring match).
    pub fn search_burgs(&self, query: &str) -> Vec<&Burg> {
        let query_lower = query.to_lowercase();
        self.map
            .pack
            .burgs
            .iter()
            .skip(1)
            .filter(|b| !b.name.is_empty() && b.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Get a state by exact name (case-insensitive). Skips sentinel at index 0.
    pub fn get_state(&self, name: &str) -> Option<&State> {
        let name_lower = name.to_lowercase();
        self.map
            .pack
            .states
            .iter()
            .skip(1)
            .find(|s| s.name.to_lowercase() == name_lower)
    }

    /// Get all burgs belonging to a state (by state name, case-insensitive).
    pub fn burgs_in_state(&self, state_name: &str) -> Vec<&Burg> {
        let state = match self.get_state(state_name) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let state_id = state.i;
        self.map
            .pack
            .burgs
            .iter()
            .skip(1)
            .filter(|b| b.state == state_id)
            .collect()
    }

    /// Find burgs near a given burg within a radius (Euclidean distance).
    /// Returns a list of (burg, distance) pairs sorted by distance.
    pub fn nearby_burgs(&self, burg_name: &str, radius: f64) -> Vec<(&Burg, f64)> {
        let origin = match self.get_burg(burg_name) {
            Some(b) => b,
            None => return Vec::new(),
        };
        let ox = origin.x;
        let oy = origin.y;

        let mut results: Vec<(&Burg, f64)> = self
            .map
            .pack
            .burgs
            .iter()
            .skip(1)
            .filter(|b| !b.name.is_empty() && b.name != origin.name)
            .filter_map(|b| {
                let dist = ((b.x - ox).powi(2) + (b.y - oy).powi(2)).sqrt();
                if dist <= radius {
                    Some((b, dist))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Find a route connecting two burgs. Returns the first route whose
    /// start and end points are closest to the given burgs.
    pub fn get_route(&self, from: &str, to: &str) -> Option<&Route> {
        let from_burg = self.get_burg(from)?;
        let to_burg = self.get_burg(to)?;

        // Find the route whose endpoints are closest to both burgs
        self.map.pack.routes.iter().find(|route| {
            if route.points.len() < 2 {
                return false;
            }
            let start = &route.points[0];
            let end = &route.points[route.points.len() - 1];

            let start_to_from =
                ((start.x - from_burg.x).powi(2) + (start.y - from_burg.y).powi(2)).sqrt();
            let end_to_to =
                ((end.x - to_burg.x).powi(2) + (end.y - to_burg.y).powi(2)).sqrt();

            let threshold = 50.0; // tolerance for matching route endpoints to burgs
            (start_to_from < threshold && end_to_to < threshold)
                || (((start.x - to_burg.x).powi(2) + (start.y - to_burg.y).powi(2)).sqrt()
                    < threshold
                    && ((end.x - from_burg.x).powi(2) + (end.y - from_burg.y).powi(2)).sqrt()
                        < threshold)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::azgaar::parse_azgaar_json;

    fn test_map() -> WorldMap {
        let json = r##"{
            "pack": {
                "burgs": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Silverport", "x": 100.0, "y": 100.0, "population": 28.5, "state": 1, "type": "City", "capital": 1},
                    {"i": 2, "name": "Ironhold", "x": 200.0, "y": 100.0, "population": 12.0, "state": 1, "type": "Town"},
                    {"i": 3, "name": "Darkwatch", "x": 500.0, "y": 500.0, "population": 5.0, "state": 2, "type": "Village"},
                    {"i": 4, "name": "Silver Lake", "x": 120.0, "y": 110.0, "population": 3.0, "state": 1}
                ],
                "states": [
                    {"i": 0, "name": ""},
                    {"i": 1, "name": "Kingdom of Light", "form": "Monarchy"},
                    {"i": 2, "name": "Shadow Realm", "form": "Theocracy"}
                ],
                "routes": [
                    {"i": 1, "points": [{"x": 100.0, "y": 100.0}, {"x": 150.0, "y": 100.0}, {"x": 200.0, "y": 100.0}], "group": "roads", "length": 100.0}
                ]
            }
        }"##;
        WorldMap::from_parsed(parse_azgaar_json(json).unwrap())
    }

    #[test]
    fn test_get_burg_found() {
        let world = test_map();
        let burg = world.get_burg("Silverport").unwrap();
        assert_eq!(burg.name, "Silverport");
        assert_eq!(burg.population, 28.5);
    }

    #[test]
    fn test_get_burg_case_insensitive() {
        let world = test_map();
        assert!(world.get_burg("silverport").is_some());
        assert!(world.get_burg("IRONHOLD").is_some());
    }

    #[test]
    fn test_get_burg_not_found() {
        let world = test_map();
        assert!(world.get_burg("Nowhere").is_none());
    }

    #[test]
    fn test_search_burgs() {
        let world = test_map();
        let results = world.search_burgs("silver");
        assert_eq!(results.len(), 2); // Silverport + Silver Lake
        assert!(results.iter().any(|b| b.name == "Silverport"));
        assert!(results.iter().any(|b| b.name == "Silver Lake"));
    }

    #[test]
    fn test_search_burgs_no_match() {
        let world = test_map();
        let results = world.search_burgs("unicorn");
        assert!(results.is_empty());
    }

    #[test]
    fn test_get_state() {
        let world = test_map();
        let state = world.get_state("Kingdom of Light").unwrap();
        assert_eq!(state.form, "Monarchy");
    }

    #[test]
    fn test_get_state_not_found() {
        let world = test_map();
        assert!(world.get_state("Nowhere Land").is_none());
    }

    #[test]
    fn test_burgs_in_state() {
        let world = test_map();
        let burgs = world.burgs_in_state("Kingdom of Light");
        assert_eq!(burgs.len(), 3); // Silverport, Ironhold, Silver Lake
        assert!(burgs.iter().all(|b| b.state == 1));
    }

    #[test]
    fn test_burgs_in_state_unknown() {
        let world = test_map();
        let burgs = world.burgs_in_state("Fake State");
        assert!(burgs.is_empty());
    }

    #[test]
    fn test_nearby_burgs_sorted_by_distance() {
        let world = test_map();
        // Silverport is at (100, 100)
        // Silver Lake is at (120, 110) — ~22 distance
        // Ironhold is at (200, 100) — 100 distance
        // Darkwatch is at (500, 500) — ~566 distance
        let nearby = world.nearby_burgs("Silverport", 150.0);
        assert_eq!(nearby.len(), 2); // Silver Lake and Ironhold
        assert_eq!(nearby[0].0.name, "Silver Lake"); // closer
        assert_eq!(nearby[1].0.name, "Ironhold"); // farther
        assert!(nearby[0].1 < nearby[1].1); // sorted by distance
    }

    #[test]
    fn test_nearby_burgs_unknown_origin() {
        let world = test_map();
        let nearby = world.nearby_burgs("Nowhere", 500.0);
        assert!(nearby.is_empty());
    }

    #[test]
    fn test_get_route() {
        let world = test_map();
        // Route goes from (100,100) to (200,100) — matching Silverport to Ironhold
        let route = world.get_route("Silverport", "Ironhold");
        assert!(route.is_some());
        assert_eq!(route.unwrap().length, 100.0);
    }

    #[test]
    fn test_get_route_reversed() {
        let world = test_map();
        // Should also work in reverse
        let route = world.get_route("Ironhold", "Silverport");
        assert!(route.is_some());
    }

    #[test]
    fn test_get_route_not_found() {
        let world = test_map();
        // No route between Silverport and Darkwatch
        let route = world.get_route("Silverport", "Darkwatch");
        assert!(route.is_none());
    }

    #[test]
    fn test_get_route_unknown_burg() {
        let world = test_map();
        assert!(world.get_route("Silverport", "Nowhere").is_none());
        assert!(world.get_route("Nowhere", "Silverport").is_none());
    }
}

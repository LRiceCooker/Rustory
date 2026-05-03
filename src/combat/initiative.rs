#[derive(Debug, Clone)]
pub struct Combatant {
    pub name: String,
    pub initiative: f64,
    pub is_current: bool,
}

#[derive(Debug, Default)]
pub struct InitiativeTracker {
    combatants: Vec<Combatant>,
    current_index: usize,
}

impl InitiativeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a combatant with a given initiative value.
    pub fn add(&mut self, name: impl Into<String>, value: f64) {
        self.combatants.push(Combatant {
            name: name.into(),
            initiative: value,
            is_current: false,
        });
    }

    /// Remove a combatant by name. Adjusts current_index if needed.
    pub fn remove(&mut self, name: &str) -> bool {
        let pos = self.combatants.iter().position(|c| c.name == name);
        match pos {
            Some(idx) => {
                let was_current = self.combatants[idx].is_current;
                self.combatants.remove(idx);
                if self.combatants.is_empty() {
                    self.current_index = 0;
                } else if idx < self.current_index {
                    self.current_index -= 1;
                } else if idx == self.current_index && self.current_index >= self.combatants.len() {
                    self.current_index = 0;
                }
                // If we removed the current combatant, mark the new current
                if was_current && !self.combatants.is_empty() {
                    self.combatants[self.current_index].is_current = true;
                }
                true
            }
            None => false,
        }
    }

    /// Sort combatants by initiative descending (highest first).
    pub fn sort(&mut self) {
        // Clear current marker before sorting
        for c in &mut self.combatants {
            c.is_current = false;
        }
        self.combatants.sort_by(|a, b| {
            b.initiative
                .partial_cmp(&a.initiative)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.current_index = 0;
        if !self.combatants.is_empty() {
            self.combatants[0].is_current = true;
        }
    }

    /// Advance to the next combatant's turn. Wraps around.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&Combatant> {
        if self.combatants.is_empty() {
            return None;
        }
        self.combatants[self.current_index].is_current = false;
        self.current_index = (self.current_index + 1) % self.combatants.len();
        self.combatants[self.current_index].is_current = true;
        Some(&self.combatants[self.current_index])
    }

    /// Go back to the previous combatant's turn. Wraps around.
    pub fn prev(&mut self) -> Option<&Combatant> {
        if self.combatants.is_empty() {
            return None;
        }
        self.combatants[self.current_index].is_current = false;
        if self.current_index == 0 {
            self.current_index = self.combatants.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.combatants[self.current_index].is_current = true;
        Some(&self.combatants[self.current_index])
    }

    /// Get the current combatant.
    pub fn current(&self) -> Option<&Combatant> {
        if self.combatants.is_empty() {
            return None;
        }
        Some(&self.combatants[self.current_index])
    }

    /// Get all combatants in initiative order.
    pub fn all(&self) -> &[Combatant] {
        &self.combatants
    }

    /// Check if the tracker is empty.
    pub fn is_empty(&self) -> bool {
        self.combatants.is_empty()
    }

    /// Number of combatants.
    pub fn len(&self) -> usize {
        self.combatants.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_combatants() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("Thorin", 18.0);
        tracker.add("Goblin", 12.0);
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.all()[0].name, "Thorin");
        assert_eq!(tracker.all()[1].name, "Goblin");
    }

    #[test]
    fn test_sort_by_initiative_descending() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("Goblin", 12.0);
        tracker.add("Thorin", 18.0);
        tracker.add("Elara", 15.0);
        tracker.sort();

        assert_eq!(tracker.all()[0].name, "Thorin");
        assert_eq!(tracker.all()[0].initiative, 18.0);
        assert_eq!(tracker.all()[1].name, "Elara");
        assert_eq!(tracker.all()[2].name, "Goblin");
        // After sort, first combatant is current
        assert!(tracker.all()[0].is_current);
    }

    #[test]
    fn test_next_cycles_through() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        tracker.add("B", 15.0);
        tracker.add("C", 10.0);
        tracker.sort();

        // Currently on A (index 0 after sort)
        assert_eq!(tracker.current().unwrap().name, "A");

        // Next -> B
        let b = tracker.next().unwrap();
        assert_eq!(b.name, "B");
        assert!(b.is_current);

        // Next -> C
        let c = tracker.next().unwrap();
        assert_eq!(c.name, "C");

        // Next -> wraps to A
        let a = tracker.next().unwrap();
        assert_eq!(a.name, "A");
    }

    #[test]
    fn test_prev_goes_back() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        tracker.add("B", 15.0);
        tracker.add("C", 10.0);
        tracker.sort();

        // Currently on A (index 0)
        assert_eq!(tracker.current().unwrap().name, "A");

        // Prev -> wraps to C
        let c = tracker.prev().unwrap();
        assert_eq!(c.name, "C");

        // Prev -> B
        let b = tracker.prev().unwrap();
        assert_eq!(b.name, "B");

        // Prev -> A
        let a = tracker.prev().unwrap();
        assert_eq!(a.name, "A");
    }

    #[test]
    fn test_remove_mid_combat() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        tracker.add("B", 15.0);
        tracker.add("C", 10.0);
        tracker.sort();

        // Move to B
        tracker.next();
        assert_eq!(tracker.current().unwrap().name, "B");

        // Remove B (the current combatant)
        assert!(tracker.remove("B"));
        assert_eq!(tracker.len(), 2);

        // Current should now be C (same index, B was removed)
        assert_eq!(tracker.current().unwrap().name, "C");
        assert!(tracker.current().unwrap().is_current);
    }

    #[test]
    fn test_remove_before_current() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        tracker.add("B", 15.0);
        tracker.add("C", 10.0);
        tracker.sort();

        // Move to C (index 2)
        tracker.next(); // B
        tracker.next(); // C
        assert_eq!(tracker.current().unwrap().name, "C");

        // Remove A (before current) — current_index should shift
        assert!(tracker.remove("A"));
        assert_eq!(tracker.current().unwrap().name, "C");
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        assert!(!tracker.remove("Nobody"));
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_empty_tracker() {
        let tracker = InitiativeTracker::new();
        assert!(tracker.is_empty());
        assert!(tracker.current().is_none());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_next_on_empty_returns_none() {
        let mut tracker = InitiativeTracker::new();
        assert!(tracker.next().is_none());
    }

    #[test]
    fn test_prev_on_empty_returns_none() {
        let mut tracker = InitiativeTracker::new();
        assert!(tracker.prev().is_none());
    }

    #[test]
    fn test_remove_last_combatant() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("A", 20.0);
        tracker.sort();
        assert!(tracker.remove("A"));
        assert!(tracker.is_empty());
        assert!(tracker.current().is_none());
    }

    #[test]
    fn test_sort_sets_first_as_current() {
        let mut tracker = InitiativeTracker::new();
        tracker.add("Low", 5.0);
        tracker.add("High", 25.0);
        tracker.add("Mid", 15.0);
        tracker.sort();

        let current = tracker.current().unwrap();
        assert_eq!(current.name, "High");
        assert!(current.is_current);
        assert!(!tracker.all()[1].is_current);
        assert!(!tracker.all()[2].is_current);
    }
}

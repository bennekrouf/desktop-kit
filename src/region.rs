//! The AI regions api0 offers, named the same way in every app.

/// One of the AI regions api0 offers. Picking it sends its id as the model;
/// api0 decides what runs there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Region {
    pub id: &'static str,
    pub flag: &'static str,
    pub label: &'static str,
    /// Flag and name, e.g. "🇪🇺 Europe".
    pub display: &'static str,
}

impl Region {
    /// What text shows for this region: flag and name, or the name alone on
    /// Windows, which draws flag emoji as two bare letters.
    pub fn shown(&self) -> &'static str {
        if cfg!(windows) {
            self.label
        } else {
            self.display
        }
    }
}

/// In the order Settings shows them.
// One line per region on purpose: this reads as a table, and rustfmt would
// spread each entry over five lines.
#[rustfmt::skip]
pub const REGIONS: &[Region] = &[
    Region { id: "europe", flag: "🇪🇺", label: "Europe", display: "🇪🇺 Europe" },
    Region { id: "china", flag: "🇨🇳", label: "China", display: "🇨🇳 China" },
    Region { id: "usa", flag: "🇺🇸", label: "United States", display: "🇺🇸 United States" },
];

/// The region with this id.
pub fn find(id: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.id == id.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_region_shows_its_flag_before_its_name() {
        for r in REGIONS {
            assert_eq!(r.display, format!("{} {}", r.flag, r.label));
        }
    }

    #[test]
    fn regions_are_found_by_id() {
        assert_eq!(find("china").map(|r| r.label), Some("China"));
        assert_eq!(find(" usa ").map(|r| r.label), Some("United States"));
        assert_eq!(find("mars"), None);
    }
}

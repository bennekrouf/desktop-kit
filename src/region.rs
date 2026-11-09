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

/// Where a new install starts: open when the others are not. Change it if
/// api0's first open region changes.
pub const DEFAULT: &Region = &REGIONS[1];

/// Said on a region's flag while api0 has not opened it.
pub const COMING_SOON: &str = "Coming soon";

/// The ids in a `/v1/regions` reply that cannot be chosen yet.
fn parse_closed(body: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    v["regions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["available"] == false)
        .filter_map(|r| r["id"].as_str().map(str::to_string))
        .collect()
}

/// The regions api0 has not opened yet, asked with no licence. Empty when it
/// cannot be asked: the flags stay clickable, and api0 still refuses a closed
/// one, rather than offline showing everything as Coming soon.
pub async fn closed() -> Vec<String> {
    fetch_closed().await.unwrap_or_default()
}

async fn fetch_closed() -> Option<Vec<String>> {
    let url = format!("{}/v1/regions", crate::licence::GATEWAY);
    crate::http::get_ok(&url, "regions", std::time::Duration::from_secs(10))
        .await
        .map(|body| parse_closed(&body))
}

/// Why `region` cannot be switched to, if it is closed.
fn refusal(region: &Region, closed: &[String]) -> Option<String> {
    closed.iter().any(|c| c == region.id).then(|| {
        let open: Vec<&str> = REGIONS
            .iter()
            .map(|r| r.id)
            .filter(|id| !closed.iter().any(|c| c == id))
            .collect();
        match open.as_slice() {
            [] => format!("{} is coming soon; no region is open now", region.label),
            open => format!(
                "{} is coming soon; open now: {}",
                region.label,
                open.join(", ")
            ),
        }
    })
}

/// For what switches a region with no flags to grey out, such as an MCP tool.
/// Stricter than [`closed`]: if api0 cannot be asked, the switch is refused.
pub async fn ensure_open(region: &Region) -> Result<(), String> {
    let closed = fetch_closed().await.ok_or_else(|| {
        format!(
            "could not ask api0 whether {} is open; try again",
            region.label
        )
    })?;
    refusal(region, &closed).map_or(Ok(()), Err)
}

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
    fn the_new_install_region_is_china() {
        assert_eq!(DEFAULT.id, "china");
    }

    #[test]
    fn only_a_region_said_to_be_unavailable_is_closed() {
        let body = r#"{"regions":[{"id":"europe","available":false},{"id":"china","available":true},{"id":"usa","available":false},{"id":"mars"}]}"#;
        assert_eq!(parse_closed(body), ["europe", "usa"]);
        assert!(parse_closed("<html>").is_empty());
    }

    #[test]
    fn a_closed_region_is_refused_naming_what_is_open() {
        let china = find("china").unwrap();
        let europe = find("europe").unwrap();
        let closed = ["europe".to_string(), "usa".to_string()];
        assert_eq!(refusal(china, &closed), None);
        assert_eq!(
            refusal(europe, &closed).unwrap(),
            "Europe is coming soon; open now: china"
        );
        let all: Vec<String> = REGIONS.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(
            refusal(china, &all).unwrap(),
            "China is coming soon; no region is open now"
        );
    }

    #[test]
    fn regions_are_found_by_id() {
        assert_eq!(find("china").map(|r| r.label), Some("China"));
        assert_eq!(find(" usa ").map(|r| r.label), Some("United States"));
        assert_eq!(find("mars"), None);
    }
}

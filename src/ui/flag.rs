//! The flag of an api0 region, drawn rather than an emoji: Windows has no flag
//! emoji and shows two letters instead.

use dioxus::prelude::*;

/// A region's flag, by [`crate::region::Region::id`]; grey for an id it doesn't know.
#[component]
pub fn RegionFlag(id: &'static str) -> Element {
    // A five-pointed star of radius 1 around the origin.
    const STAR: &str = "0,-1 0.24,-0.32 0.95,-0.31 0.38,0.12 0.59,0.81 0,0.4 \
                        -0.59,0.81 -0.38,0.12 -0.95,-0.31 -0.24,-0.32";
    let flag = match id {
        "europe" => rsx! {
            rect { width: "30", height: "20", fill: "#003399" }
            // Twelve stars on a circle, as dots: a star this small is a dot.
            for i in 0..12 {
                circle {
                    cx: "{15.0 + 6.0 * (f64::from(i) * std::f64::consts::PI / 6.0).sin()}",
                    cy: "{10.0 - 6.0 * (f64::from(i) * std::f64::consts::PI / 6.0).cos()}",
                    r: "0.9",
                    fill: "#FFCC00",
                }
            }
        },
        "china" => rsx! {
            rect { width: "30", height: "20", fill: "#EE1C25" }
            polygon { points: STAR, fill: "#FFFF00", transform: "translate(5 5) scale(3)" }
            for (x, y) in [(10.0, 2.0), (12.0, 4.0), (12.0, 7.0), (10.0, 9.0)] {
                polygon { points: STAR, fill: "#FFFF00", transform: "translate({x} {y})" }
            }
        },
        "usa" => rsx! {
            rect { width: "30", height: "20", fill: "#fff" }
            for i in 0..7 {
                rect { y: "{f64::from(i) * 40.0 / 13.0}", width: "30", height: "{20.0 / 13.0}", fill: "#B22234" }
            }
            rect { width: "12", height: "{140.0 / 13.0}", fill: "#3C3B6E" }
        },
        _ => rsx! { rect { width: "30", height: "20", fill: "#888" } },
    };
    rsx! {
        svg { class: "kit-flag", view_box: "0 0 30 20", "aria-hidden": "true", {flag} }
    }
}

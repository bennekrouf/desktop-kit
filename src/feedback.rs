//! The way back to mayorana.ch from inside an app.
//!
//! People get an app as a download and may never see the site again: the
//! update banner and the notice only show when there is something to say. A
//! Feedback link is always on screen, so anyone with an idea or a problem has
//! somewhere to send it.

/// The contact form with the topic set and the app and version filled in, so a
/// message says what it is about without the sender having to. The form
/// without them before [`crate::init`].
pub fn contact_url() -> String {
    match crate::app() {
        Some(app) => contact_url_for(app.name, app.version),
        None => "https://mayorana.ch/en/contact?service=tools".to_string(),
    }
}

/// The app's page on the site: releases, docs, the other apps.
pub fn site_url() -> String {
    match crate::app() {
        Some(app) => format!("https://mayorana.ch/en/apps/{}", app.id),
        None => "https://mayorana.ch/en/apps".to_string(),
    }
}

fn contact_url_for(name: &str, version: &str) -> String {
    format!(
        "https://mayorana.ch/en/contact?service=tools&app={}&v={}",
        name.replace(' ', "%20"),
        version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_knows_the_app_and_its_version() {
        assert_eq!(
            contact_url_for("TrackCut", "0.1.37"),
            "https://mayorana.ch/en/contact?service=tools&app=TrackCut&v=0.1.37"
        );
        assert_eq!(
            contact_url_for("Surface Watch", "1.0.0"),
            "https://mayorana.ch/en/contact?service=tools&app=Surface%20Watch&v=1.0.0"
        );
    }
}

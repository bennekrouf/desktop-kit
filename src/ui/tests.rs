use dioxus::prelude::*;

use super::*;

fn no_get(
    _: crate::http::Request,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<crate::http::Response>> + Send>> {
    Box::pin(async { None })
}

/// The same name and version as the telemetry tests' app: whichever test
/// names the app first, the header shows these.
fn named() {
    crate::init(crate::App {
        id: "desktop-kit-test",
        name: "Desktop Kit",
        version: "0.0.1",
        telemetry_folder: "desktop-kit-test",
        http_get: no_get,
    });
}

#[derive(Props, Clone, PartialEq)]
struct Bar {
    light: bool,
    settings: bool,
    mcp: bool,
    look: Option<ProLook>,
}

#[allow(non_snake_case)]
fn Harness(bar: Bar) -> Element {
    let theme = Theme {
        light: use_signal(|| bar.light),
        picked: use_signal(|| false),
    };
    let on_settings = bar.settings.then(|| EventHandler::new(|_| {}));
    let on_mcp = bar.mcp.then(|| EventHandler::new(|_| {}));
    let pro = bar
        .look
        .map(|look| rsx! { ProBadge { look, title: "licence", onclick: |_| {} } });
    rsx! {
        Header {
            theme,
            on_settings,
            on_mcp,
            pro,
            model: rsx! { ModelBadge { label: "Europe", region: "europe", title: "model", onclick: |_| {} } },
            span { class: "kit-detail", "~/code/site" }
        }
    }
}

fn render(bar: Bar) -> String {
    named();
    let mut dom = VirtualDom::new_with_props(Harness, bar);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn bar() -> Bar {
    Bar {
        light: false,
        settings: false,
        mcp: false,
        look: None,
    }
}

#[test]
fn the_bar_names_the_app_and_its_version() {
    let html = render(bar());
    assert!(
        html.contains(r#"<span class="kit-name">Desktop Kit</span>"#),
        "{html}"
    );
    assert!(html.contains(">v0.0.1<"), "{html}");
    assert!(html.contains("~/code/site"), "{html}");
    assert!(
        html.contains("kit-flag") && html.contains("Europe"),
        "{html}"
    );
}

#[test]
fn feedback_is_always_there_settings_and_mcp_only_when_they_open_something() {
    let html = render(bar());
    assert!(html.contains(">Feedback<"), "{html}");
    assert!(html.contains("app=Desktop%20Kit"), "{html}");
    assert!(
        !html.contains(">MCP<") && !html.contains(">Settings<"),
        "{html}"
    );

    let html = render(Bar {
        settings: true,
        mcp: true,
        ..bar()
    });
    assert!(
        html.contains(">MCP<") && html.contains(">Settings<"),
        "{html}"
    );
    assert!(
        html.find(">Settings<") < html.find(">MCP<")
            && html.find(">MCP<") < html.find(">Feedback<"),
        "Settings, MCP, Feedback in that order: {html}"
    );
}

#[test]
fn the_licence_sits_last_in_the_corner() {
    let html = render(Bar {
        look: Some(ProLook::Pro),
        ..bar()
    });
    let pro = html.find("PRO").unwrap_or(0);
    assert!(
        pro > 0 && pro > html.find(">Feedback<").unwrap_or(usize::MAX),
        "{html}"
    );
    assert!(html.contains(r#"class="kit-pro""#), "{html}");

    let html = render(Bar {
        look: Some(ProLook::Renew),
        ..bar()
    });
    assert!(
        html.contains("RENEW PRO") && html.contains("kit-pro-renew"),
        "{html}"
    );

    let html = render(Bar {
        look: Some(ProLook::Get),
        ..bar()
    });
    assert!(html.contains("Get Pro…") && !html.contains("✦"), "{html}");
}

#[test]
fn the_toggle_offers_the_other_theme() {
    assert!(render(bar()).contains(r#"title="Switch to light mode""#));
    assert!(render(Bar {
        light: true,
        ..bar()
    })
    .contains(r#"title="Switch to dark mode""#));
}

#[test]
fn a_licence_status_has_one_look() {
    use crate::licence::Status;
    assert_eq!(ProLook::of(&Status::Free), ProLook::Get);
    assert_eq!(ProLook::of(&Status::Unknown("X".into())), ProLook::Renew);
}

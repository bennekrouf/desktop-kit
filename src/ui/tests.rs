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
        product: "desktop-kit-test",
        licence_file: || None,
        http: no_get,
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

#[derive(Props, Clone, PartialEq)]
struct Window {
    open: bool,
    status: crate::licence::Status,
}

#[allow(non_snake_case)]
fn ProHarness(window: Window) -> Element {
    let pro = Pro {
        open: use_signal(|| window.open),
        status: use_signal(|| window.status.clone()),
    };
    rsx! {
        ProWindow { pro, pitch: rsx! { p { "The free version does one thing." } } }
    }
}

fn render_pro(open: bool, status: crate::licence::Status) -> String {
    named();
    let mut dom = VirtualDom::new_with_props(ProHarness, Window { open, status });
    dom.rebuild_in_place();
    without_style(dioxus_ssr::render(&dom))
}

/// The page without its stylesheets, whose class names would match too.
fn without_style(html: String) -> String {
    let mut out = String::new();
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<style>") {
        out.push_str(&rest[..start]);
        rest = rest[start..]
            .split_once("</style>")
            .map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out
}

fn answer(active: bool) -> crate::licence::Answer {
    crate::licence::Answer {
        licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(),
        email: "dev@example.ch".into(),
        plan: crate::licence::Plan {
            id: "pro".into(),
            name: "Studio".into(),
        },
        products: vec!["*".into()],
        active,
        period_end: "2026-11-21T10:00:00Z".into(),
        ..crate::licence::Answer::default()
    }
}

#[test]
fn without_a_licence_the_window_sells_pro_and_asks_for_a_number() {
    use crate::licence::Status;
    let html = render_pro(true, Status::Free);
    assert!(html.contains("Desktop Kit Pro"), "{html}");
    assert!(html.contains("The free version does one thing."), "{html}");
    assert!(html.contains("Buy Desktop Kit Pro"), "{html}");
    assert!(
        html.contains("ABCDE-FGHJK-LMNPQ-RSTUV") && html.contains(">Activate<"),
        "{html}"
    );
    assert!(html.contains("Get free AI credits"), "{html}");
    assert!(!html.contains("Remove the licence"), "{html}");
}

#[test]
fn a_pro_licence_is_congratulated_with_its_plan() {
    use crate::licence::Status;
    let html = render_pro(true, Status::Pro(answer(true)));
    assert!(html.contains("You&#39;re Pro"), "{html}");
    assert!(html.contains("Studio plan for dev@example.ch"), "{html}");
    assert!(html.contains("Manage subscription"), "{html}");
    assert!(
        !html.contains(">Activate<") && !html.contains("one thing"),
        "{html}"
    );
}

#[test]
fn a_lapsed_licence_is_renewed_rather_than_bought() {
    use crate::licence::Status;
    let html = render_pro(true, Status::NotPro(answer(false)));
    assert!(html.contains("Renew Desktop Kit Pro"), "{html}");
    assert!(html.contains("runs as the free version"), "{html}");
    assert!(
        html.contains("Check again") && html.contains("Remove the licence"),
        "{html}"
    );
    assert!(!html.contains(">Activate<"), "{html}");
}

#[test]
fn a_closed_window_shows_nothing() {
    let html = render_pro(false, crate::licence::Status::Free);
    assert!(!html.contains("kit-pro"), "{html}");
}

#[allow(non_snake_case)]
fn BannerHarness() -> Element {
    rsx! { Banners { statistics: false } }
}

#[test]
fn banners_show_nothing_until_there_is_something_to_say() {
    named();
    let mut dom = VirtualDom::new(BannerHarness);
    dom.rebuild_in_place();
    let html = without_style(dioxus_ssr::render(&dom));
    assert!(html.contains("kit-banners"), "{html}");
    assert!(
        !html.contains("kit-banner\"") && !html.contains("kit-consent"),
        "{html}"
    );
}

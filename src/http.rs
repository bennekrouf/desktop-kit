//! The network calls the shared modules make, done by the app.
//!
//! The apps use different clients (reqwest in most, ureq in PolishCast and
//! TrackCut) with different TLS stacks. Passing the call in keeps this crate
//! from adding a second client, or a second TLS stack, to any of them.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// What to send.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub url: String,
    /// `"<app id>/<version> (<purpose>)"`. The download statistics tell an
    /// update check, a notice fetch and a telemetry send apart by it.
    pub user_agent: String,
    /// The whole request, connecting included.
    pub timeout: Duration,
    /// More headers, e.g. a licence's `Authorization`.
    pub headers: Vec<(&'static str, String)>,
    /// A JSON body, sent as `application/json`.
    pub json: Option<String>,
}

/// What came back. Any status: the caller decides what counts as success.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

/// The app's request: `None` for any failure to get an answer at all (no
/// network, timeout, refused). Must not panic.
///
/// With reqwest:
///
/// ```ignore
/// fn http(req: Request) -> Pin<Box<dyn Future<Output = Option<Response>> + Send>> {
///     Box::pin(async move {
///         let client = reqwest::Client::new();
///         let mut r = match req.method {
///             Method::Get => client.get(&req.url),
///             Method::Post => client.post(&req.url),
///         };
///         r = r.header(reqwest::header::USER_AGENT, req.user_agent).timeout(req.timeout);
///         for (name, value) in req.headers {
///             r = r.header(name, value);
///         }
///         if let Some(json) = req.json {
///             r = r.header(reqwest::header::CONTENT_TYPE, "application/json").body(json);
///         }
///         let r = r.send().await.ok()?;
///         let status = r.status().as_u16();
///         Some(Response { status, body: r.text().await.ok()? })
///     })
/// }
/// ```
pub type Call = fn(Request) -> Pin<Box<dyn Future<Output = Option<Response>> + Send>>;

/// A request through the app's client, under the app's User-Agent. `None`
/// before [`crate::init`].
pub(crate) async fn send(
    method: Method,
    url: &str,
    purpose: &str,
    timeout: Duration,
    headers: Vec<(&'static str, String)>,
    json: Option<String>,
) -> Option<Response> {
    let app = crate::app()?;
    let request = Request {
        method,
        url: url.to_string(),
        user_agent: format!("{}/{} ({purpose})", app.id, app.version),
        timeout,
        headers,
        json,
    };
    (app.http)(request).await
}

/// A GET through the app's client.
pub(crate) async fn get(url: &str, purpose: &str, timeout: Duration) -> Option<Response> {
    send(Method::Get, url, purpose, timeout, Vec::new(), None).await
}

/// The same, for a 2xx answer only.
pub(crate) async fn get_ok(url: &str, purpose: &str, timeout: Duration) -> Option<String> {
    get(url, purpose, timeout)
        .await
        .filter(|r| (200..300).contains(&r.status))
        .map(|r| r.body)
}

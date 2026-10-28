//! The one network call the shared modules make: a GET, done by the app.
//!
//! The apps use different clients (reqwest in most, ureq in PolishCast and
//! TrackCut) with different TLS stacks. Passing the call in keeps this crate
//! from adding a second client, or a second TLS stack, to any of them.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// What to fetch.
#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,
    /// `"<app id>/<version> (<purpose>)"`. The download statistics tell an
    /// update check, a notice fetch and a telemetry send apart by it.
    pub user_agent: String,
    /// The whole request, connecting included.
    pub timeout: Duration,
}

/// What came back. Any status: the caller decides what counts as success.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

/// The app's GET: `None` for any failure to get an answer at all (no network,
/// timeout, refused). Must not panic.
///
/// With reqwest:
///
/// ```ignore
/// fn http_get(req: Request) -> Pin<Box<dyn Future<Output = Option<Response>> + Send>> {
///     Box::pin(async move {
///         let r = reqwest::Client::new()
///             .get(&req.url)
///             .header(reqwest::header::USER_AGENT, req.user_agent)
///             .timeout(req.timeout)
///             .send()
///             .await
///             .ok()?;
///         let status = r.status().as_u16();
///         Some(Response { status, body: r.text().await.ok()? })
///     })
/// }
/// ```
pub type Get = fn(Request) -> Pin<Box<dyn Future<Output = Option<Response>> + Send>>;

/// A GET through the app's client, under the app's User-Agent. `None` before
/// [`crate::init`].
pub(crate) async fn get(url: &str, purpose: &str, timeout: Duration) -> Option<Response> {
    let app = crate::app()?;
    let request = Request {
        url: url.to_string(),
        user_agent: format!("{}/{} ({purpose})", app.id, app.version),
        timeout,
    };
    (app.http_get)(request).await
}

/// The same, for a 2xx answer only.
pub(crate) async fn get_ok(url: &str, purpose: &str, timeout: Duration) -> Option<String> {
    get(url, purpose, timeout)
        .await
        .filter(|r| (200..300).contains(&r.status))
        .map(|r| r.body)
}

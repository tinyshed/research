// Added by Dashbin, see UPSTREAM; Apache-2.0 like the rest of this crate.
// A network policy per runtime: which hosts it may name, and a client whose resolver refuses private addresses.
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Uri};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::connect::dns::Name;
use hyper_util::client::legacy::{Client, ResponseFuture};
use hyper_util::rt::{TokioExecutor, TokioTimer};
use rquickjs::{Ctx, Exception, JsLifetime, Result};
use tower_service::Service;

type PolicyClient = Client<HttpsConnector<HttpConnector<Resolver>>, BoxBody<Bytes, Infallible>>;

pub struct Policy {
    allow: Option<Vec<String>>,
    private: Arc<Vec<String>>,
    client: PolicyClient,
    contacted: Mutex<BTreeMap<String, u64>>,
}

#[derive(Clone)]
struct Handle(Arc<Policy>);

unsafe impl<'js> JsLifetime<'js> for Handle {
    type Changed<'to> = Handle;
}

/// Gives the runtime of `ctx` its own policy and client. `allow` of `None` is any public host;
/// `private` names the hosts an operator typed in, which may resolve to private addresses but never link-local.
pub fn set(
    ctx: &Ctx<'_>,
    allow: Option<Vec<String>>,
    private: Vec<String>,
) -> std::result::Result<Arc<Policy>, Box<dyn std::error::Error + Send + Sync>> {
    let private = Arc::new(private.into_iter().map(|h| h.to_ascii_lowercase()).collect::<Vec<_>>());
    let mut http = HttpConnector::new_with_resolver(Resolver { private: private.clone() });
    http.enforce_http(false);
    let tls = match &*llrt_tls::TLS_CONFIG {
        Ok(config) => config.clone(),
        Err(e) => return Err(e.to_string().into()),
    };
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_or_http()
        .enable_http1()
        .wrap_connector(http);
    let client = Client::builder(TokioExecutor::new()).pool_timer(TokioTimer::new()).build(https);
    let allow = allow.map(|hosts| hosts.into_iter().map(|h| h.to_ascii_lowercase()).collect());
    let policy = Arc::new(Policy { allow, private, client, contacted: Mutex::default() });
    let _ = ctx.store_userdata(Handle(policy.clone()));
    Ok(policy)
}

/// A refusal made below hyper, by the resolver, reaches the plugin in its own words rather than as `client error (Connect)`.
pub(crate) fn explain<T>(ctx: &Ctx<'_>, result: std::result::Result<T, hyper_util::client::legacy::Error>) -> Result<T> {
    result.map_err(|e| {
        let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&e);
        while let Some(c) = cause {
            let text = c.to_string();
            if text.starts_with("dashbin:") {
                return Exception::throw_message(ctx, &text);
            }
            cause = c.source();
        }
        Exception::throw_message(ctx, &e.to_string())
    })
}

pub(crate) fn of(ctx: &Ctx<'_>) -> Option<Arc<Policy>> {
    ctx.userdata::<Handle>().map(|handle| handle.0.clone())
}

impl Policy {
    /// Every host the runtime asked for, redirects included, and how often.
    pub fn contacted(&self) -> BTreeMap<String, u64> {
        self.contacted.lock().unwrap().clone()
    }

    pub(crate) fn request(&self, req: Request<BoxBody<Bytes, Infallible>>) -> ResponseFuture {
        self.client.request(req)
    }

    pub(crate) fn check(&self, ctx: &Ctx<'_>, uri: &Uri) -> Result<()> {
        let host = uri.host().unwrap_or_default().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
        *self.contacted.lock().unwrap().entry(host.clone()).or_default() += 1;
        let typed_by_operator = self.private.iter().any(|h| *h == host);
        if let Some(allow) = &self.allow {
            if !typed_by_operator && !allow.iter().any(|rule| matches(rule, &host)) {
                return Err(Exception::throw_message(ctx, &format!("dashbin: {host} is not a host this plugin may reach")));
            }
        }
        // an address written as digits never reaches the resolver, so it is judged here
        if let Ok(ip) = host.parse::<IpAddr>() {
            if !(public(ip) || typed_by_operator && !link_local(ip)) {
                return Err(Exception::throw_message(ctx, &format!("dashbin: {host} is a private address")));
            }
        }
        Ok(())
    }
}

fn matches(rule: &str, host: &str) -> bool {
    match rule.strip_prefix("*.") {
        Some(domain) => host.len() > domain.len() + 1 && host.ends_with(domain) && host.as_bytes()[host.len() - domain.len() - 1] == b'.',
        None => host == rule,
    }
}

fn link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unicast_link_local() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_link_local()),
    }
}

fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            let shared = a == 100 && (b & 0xc0) == 64;
            !(a == 0 || shared || v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_broadcast() || v4.is_multicast())
        },
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => public(IpAddr::V4(v4)),
            None => !(v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local() || v6.is_unicast_link_local() || v6.is_multicast()),
        },
    }
}

#[derive(Clone)]
pub struct Resolver {
    private: Arc<Vec<String>>,
}

impl Service<Name> for Resolver {
    type Response = std::vec::IntoIter<SocketAddr>;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = io::Result<Self::Response>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        let host = name.as_str().to_ascii_lowercase();
        let typed_by_operator = self.private.iter().any(|h| *h == host);
        Box::pin(async move {
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            // the addresses judged are the ones connected to, so a name that changes its answer gains nothing
            let allowed: Vec<SocketAddr> = found
                .into_iter()
                .filter(|a| public(a.ip()) || typed_by_operator && !link_local(a.ip()))
                .collect();
            if allowed.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("dashbin: {host} resolves only to addresses this plugin may not reach"),
                ));
            }
            Ok(allowed.into_iter())
        })
    }
}

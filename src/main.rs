use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Context;
use astra::{Request, Response, Service};
use matchit::Match;
use rate_limit::{conventional, pre};
use tracing::info;
use tracing_subscriber::EnvFilter;

type Router = matchit::Router<for<'a> fn(&Server, &'a Request, &'a matchit::Params) -> Response>;

mod page;
mod rate_limit;
mod routes;

#[derive(Default)]
struct State {
    count: AtomicU64,
}

#[derive(Clone)]
pub struct Server {
    state: Arc<State>,
    db: Arc<redb::Database>,
    router: Router,
    pre_rate_limiter: pre::FastPreRateLimiter,
    rate_limiter: conventional::RateLimiter,
}

impl Server {
    fn new() -> anyhow::Result<Self> {
        let router = {
            let mut router = Router::new();
            router.insert("/", Self::home)?;
            router.insert("/favicon.ico", Self::favicon_ico)?;
            router.insert("/style.css", Self::style_css)?;
            router.insert("/count", Self::count)?;
            router.insert("/user/:id", Self::get_user)?;
            router
        };

        let db = redb::Database::create("./target/db.redb")?;

        Ok(Self {
            state: Default::default(),
            router,
            db: db.into(),
            pre_rate_limiter: pre::FastPreRateLimiter::new(20, 60),
            rate_limiter: conventional::RateLimiter::new(10, 60),
        })
    }

    fn route(&self, req: &Request) -> Response {
        // Try to find the handler for the requested path
        match self.router.at(req.uri().path()) {
            // If a handler is found, insert the route parameters into the request
            // extensions, and call it
            Ok(Match { value, params }) => {
                let params = params.clone();
                (value)(self, req, &params)
            }
            // Otherwise return a 404
            Err(_) => self.not_found_404(req),
        }
    }
}

impl Service for Server {
    fn call(&self, req: Request, info: astra::ConnectionInfo) -> Response {
        let peer_addr = info.peer_addr();
        let peer_addr = peer_addr.unwrap_or(std::net::SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            0,
        )));
        let peer_ip = peer_addr.ip();
        let resp =
            if self.pre_rate_limiter.rate_limit(peer_ip) && self.rate_limiter.rate_limit(peer_ip) {
                self.too_many_requests_429(&req)
            } else {
                self.route(&req)
            };

        info!(
            status = %resp.status(),
            method = %req.method(),
            path = %req.uri(),
            peer = %peer_addr,
            "request"
        );
        resp
    }
}

fn main() -> anyhow::Result<()> {
    init_logging();

    let server = Server::new()?;

    astra::Server::bind("localhost:3000")
        .serve(server)
        .context("bind http server")?;

    Ok(())
}

fn init_logging() {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // Print to stderr
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

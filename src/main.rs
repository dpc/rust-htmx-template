mod fragment;
mod opts;
mod rate_limit;
mod routes;
mod util;

use std::net::{self, Ipv4Addr};
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use hyper::http::HeaderValue;
use hyper::{header, Method};
use lettre::message::{Mailbox, MessageBuilder};
use lettre::{Address, SmtpTransport, Transport};
use matchit::Match;
use rate_limit::{conventional, pre};
use tracing::info;
use tracing_subscriber::EnvFilter;

type Router = matchit::Router<
    &'static [(
        Method,
        for<'a> fn(&Service, &'a astra::Request, &'a matchit::Params) -> astra::Response,
    )],
>;

#[derive(Default)]
struct State {
    count: AtomicU64,
}

#[derive(Clone)]
pub struct Service {
    state: Arc<State>,
    db: Arc<redb::Database>,
    router: Router,
    pre_rate_limiter: pre::FastPreRateLimiter,
    rate_limiter: conventional::RateLimiter,
}

impl Service {
    fn new() -> anyhow::Result<Self> {
        let router = {
            let mut router = Router::new();
            router.insert("/", &[(Method::GET, Self::home)])?;
            router.insert("/favicon.ico", &[(Method::GET, Self::favicon_ico)])?;
            router.insert("/style.css", &[(Method::GET, Self::style_css)])?;
            router.insert("/count", &[(Method::POST, Self::count)])?;
            router.insert("/user/:id", &[(Method::GET, Self::get_user)])?;
            router.insert("/post/:id", &[(Method::POST, Self::save_post)])?;
            router.insert("/post/:id/edit", &[(Method::GET, Self::edit_post)])?;
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

    fn route(&self, req: &astra::Request) -> astra::Response {
        // Try to find the handler for the requested path
        match self.router.at(req.uri().path()) {
            // If a handler is found, insert the route parameters into the request
            // extensions, and call it
            Ok(Match { value, params }) => {
                if let Some((_method, f)) = value.iter().find(|(method, _)| req.method() == method)
                {
                    let params = params.clone();
                    (f)(self, req, &params)
                } else {
                    self.not_found_404(req)
                }
            }
            // Otherwise return a 404
            Err(_) => self.not_found_404(req),
        }
    }

    fn handle_session(
        &self,
        req: &astra::Request,
        f: impl FnOnce(&astra::Request) -> astra::Response,
    ) -> astra::Response {
        let mut session = None;
        for (k, v) in RequestExt(req).iter_cookies() {
            if k == "session" {
                session = Some(v.to_owned());
            }
        }
        let mut resp = f(req);

        if session.is_none() {
            resp.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_str("session=booo").expect("can't fail"),
            );
        }

        resp
    }

    fn handle_rate_limiting(
        &self,
        req: &astra::Request,
        info: &astra::ConnectionInfo,
        f: impl FnOnce(&astra::Request) -> astra::Response,
    ) -> (astra::Response, Option<net::SocketAddr>) {
        let peer_addr = info.peer_addr();
        let peer_ip = peer_addr
            .map(|s| s.ip())
            .unwrap_or(net::IpAddr::V4(Ipv4Addr::UNSPECIFIED));

        (
            if self.pre_rate_limiter.rate_limit(peer_ip) && self.rate_limiter.rate_limit(peer_ip) {
                self.too_many_requests_429(req)
            } else {
                f(req)
            },
            peer_addr,
        )
    }
}

pub struct RequestExt<'a>(&'a hyper::Request<astra::Body>);

impl<'a> RequestExt<'a> {
    fn iter_cookies(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .headers()
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .map(|s| s.trim())
            .flat_map(|s| s.split_once('='))
    }
}

impl astra::Service for Service {
    fn call(
        &self,
        req: hyper::Request<astra::Body>,
        info: astra::ConnectionInfo,
    ) -> astra::Response {
        let (resp, peer_addr) = self.handle_rate_limiting(&req, &info, |req| {
            self.handle_session(req, |req| self.route(req))
        });

        use crate::util::DisplayOption;
        info!(
            status = %resp.status(),
            method = %req.method(),
            path = %req.uri(),
            peer = %DisplayOption(peer_addr),
            "request"
        );
        resp
    }
}

fn main() -> anyhow::Result<()> {
    init_logging()?;

    if let Ok(path) = dotenv::dotenv() {
        info!(path = %path.display(), "Loaded env file");
    }

    let args = opts::Opts::parse();

    // send_email()?;

    let service = Service::new()?;

    let server = astra::Server::bind(args.listen);

    info!("Listening on {}", server.local_addr()?);
    server
        .serve(service)
        .context("Failed to start http server")?;

    Ok(())
}

fn init_logging() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // Print to stderr
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    Ok(())
}

fn send_email() -> anyhow::Result<()> {
    let smtp_hostname = std::env::var("SMTP_HOSTNAME")?;
    let smtp_port = std::env::var("SMTP_PORT")?;
    let smtp_username = std::env::var("SMTP_USER")?;
    let smtp_password = std::env::var("SMTP_PASSWORD")?;
    let smtp_to = std::env::var("SMTP_TO")?;
    let smtp_from = std::env::var("SMTP_FROM")?;

    let email = MessageBuilder::new()
        .to(Mailbox::new(None, Address::from_str(&smtp_to)?))
        .from(Mailbox::new(None, Address::from_str(&smtp_from)?))
        .subject("Test Email")
        .body("Hello from Rust!".to_owned())?;

    let mailer = SmtpTransport::relay(&smtp_hostname)?
        .port(FromStr::from_str(&smtp_port).context("Failed to parse port number")?)
        .credentials(lettre::transport::smtp::authentication::Credentials::new(
            smtp_username,
            smtp_password,
        ))
        .build();

    mailer
        .test_connection()
        .context("SMTP Connection test failed")?;

    mailer.send(&email).context("Failed to send email")?;

    Ok(())
}

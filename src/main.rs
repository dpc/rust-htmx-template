mod fragment;
mod opts;
mod rate_limit;
mod routes;
mod util;

use std::net::{self, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Context;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use clap::Parser;
use hyper::http;
use hyper::server::conn::AddrIncoming;
use lettre::message::{Mailbox, MessageBuilder};
use lettre::{Address, SmtpTransport, Transport};
use rate_limit::{conventional, pre};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Default)]
struct State {
    count: AtomicU64,
}

#[derive(Clone)]
pub struct Service {
    state: Arc<State>,
    #[allow(unused)]
    db: Arc<redb::Database>,
    // router: Router,
    pre_rate_limiter: pre::FastPreRateLimiter,
    rate_limiter: conventional::RateLimiter,
}

impl Service {
    fn new() -> anyhow::Result<Self> {
        let db = redb::Database::create("./target/db.redb")?;

        Ok(Self {
            state: Default::default(),
            db: db.into(),
            pre_rate_limiter: pre::FastPreRateLimiter::new(20, 60),
            rate_limiter: conventional::RateLimiter::new(10, 60),
        })
    }

    // fn handle_session(
    //     &self,
    //     req: &astra::Request,
    //     f: impl FnOnce(&astra::Request) -> astra::Response,
    // ) -> astra::Response { let mut session = None; for (k, v) in
    //   RequestExt(req).iter_cookies() { if k == "session" { session =
    //   Some(v.to_owned()); } } let mut resp = f(req);

    //     if session.is_none() {
    //         resp.headers_mut().insert(
    //             header::SET_COOKIE,
    //             HeaderValue::from_str("session=booo").expect("can't fail"),
    //         );
    //     }

    //     resp
    // }
}

// pub struct RequestExt<'a>(&'a hyper::Request<astra::Body>);

// impl<'a> RequestExt<'a> {
//     fn iter_cookies(&self) -> impl Iterator<Item = (&str, &str)> {
//         self.0
//             .headers()
//             .get_all(header::COOKIE)
//             .iter()
//             .filter_map(|v| v.to_str().ok())
//             .flat_map(|v| v.split(';'))
//             .map(|s| s.trim())
//             .flat_map(|s| s.split_once('='))
//     }
// }

// impl astra::Service for Service {
//     fn call(
//         &self,
//         req: hyper::Request<astra::Body>,
//         info: astra::ConnectionInfo,
//     ) -> astra::Response { let (resp, peer_addr) =
//       self.handle_rate_limiting(&req, &info, |req| { self.handle_session(req,
//       |req| self.route(req)) });

//         use crate::util::DisplayOption;
//         info!(
//             status = %resp.status(),
//             method = %req.method(),
//             path = %req.uri(),
//             peer = %DisplayOption(peer_addr),
//             "request"
//         );
//         resp
//     }
// }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging()?;

    if let Ok(path) = dotenv::dotenv() {
        info!(path = %path.display(), "Loaded env file");
    }

    let args = opts::Opts::parse();

    // send_email()?;

    let service = Service::new()?;
    let app = axum::Router::new()
        .route("/", get(routes::home))
        .route("/favicon.ico", get(routes::favicon_ico))
        .route("/style.css", get(routes::style_css))
        .route("/count", post(routes::count))
        .route("/user/:id", get(routes::get_user))
        .route("/post/:id", post(routes::save_post))
        .route("/post/:id/edit", get(routes::edit_post))
        .fallback(routes::not_found_404)
        .with_state(service.clone())
        .layer(middleware::from_fn_with_state(service, rate_limit))
        .layer(TraceLayer::new_for_http());

    let incoming = AddrIncoming::bind(&args.listen.parse()?)?;
    info!("Listening on {}", incoming.local_addr());
    hyper::server::Server::builder(incoming)
        .serve(app.into_make_service())
        .await
        .context("Failed to start http server")?;

    Ok(())
}

async fn rate_limit<B>(
    axum::extract::State(service): axum::extract::State<Service>,
    req: http::Request<B>,
    next: middleware::Next<B>,
) -> Response {
    let peer_addr = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0);
    let peer_ip = peer_addr
        .map(|s| s.ip())
        .unwrap_or(net::IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    if service.pre_rate_limiter.rate_limit(peer_ip) && service.rate_limiter.rate_limit(peer_ip) {
        routes::too_many_requests_429().await.into_response()
    } else {
        next.run(req).await
    }
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

#[allow(unused)]
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

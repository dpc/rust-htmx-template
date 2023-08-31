use std::sync::atomic::Ordering;

use astra::{Body, Request, Response, ResponseBuilder};
use hyper::{Method, StatusCode};
use maud::html;

use crate::fragment::{self, ResponseBuilderExt};
use crate::Service;

impl Service {
    pub fn count(&self, req: &Request, _: &matchit::Params) -> Response {
        let count = if req.method() == Method::POST {
            self.state.count.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            self.state.count.load(Ordering::Relaxed)
        };

        let html = html! {
            (count)
        };
        ResponseBuilder::new()
            .header("Content-Type", "text/html")
            .body(Body::new(html.into_string()))
            .unwrap()
    }

    /// GET '/'
    pub fn home(&self, _: &Request, _: &matchit::Params) -> Response {
        let html = fragment::page(
            "home",
            html! {
                article {
                    h2 { "An htmx button" }
                    p {
                        button name="foo" hx-post="/count" hx-swap="innerHTML" {
                            (self.state.count.load(Ordering::Relaxed))
                        }
                    }
                }

                (fragment::post("post-123", "A blogpost", "Lorem ipsum, something something."))
            },
        );
        ResponseBuilder::new()
            .header("Content-Type", "text/html")
            .body(Body::new(html.into_string()))
            .unwrap()
    }

    pub fn not_found_404(&self, _: &Request) -> Response {
        let html = fragment::page(
            "PAGE NOT FOUND",
            html! {
                h2 { "This page does not seem to exist, sorry!" }
                p {
                    a href="/" { "Return to the main page" }
                }
            },
        );
        response_html_not_found(html)
    }

    pub fn too_many_requests_429(&self, _: &Request) -> Response {
        ResponseBuilder::new()
            .header("Cache-Control", "no-store")
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(Body::new("Too Many Requests"))
            .unwrap()
    }

    pub fn favicon_ico(&self, _: &Request, _: &matchit::Params) -> Response {
        ResponseBuilder::new()
            .header("Content-Type", "image/gif")
            .cache_static()
            .body(Body::new(include_bytes!("../static/dpc.gif").as_slice()))
            .unwrap()
    }

    pub fn style_css(&self, _: &Request, _: &matchit::Params) -> Response {
        ResponseBuilder::new()
            .header("Content-Type", "text/css")
            .cache_static()
            .body(Body::new(include_str!("../static/style.css")))
            .unwrap()
    }

    /// GET '/user/:id'
    pub fn get_user(&self, _: &Request, params: &matchit::Params) -> Response {
        // Retrieve route parameters from the the request extensions
        let id = params.get("id").unwrap();

        Response::new(Body::new(format!("User #{id}")))
    }

    pub fn edit_post(&self, _: &Request, params: &matchit::Params) -> Response {
        // Retrieve route parameters from the the request extensions
        let id = params.get("id").unwrap();

        response_html(fragment::post_edit_form(id, "Foo", "Content"))
    }

    pub fn save_post(&self, _: &Request, params: &matchit::Params) -> Response {
        // Retrieve route parameters from the the request extensions
        let id = params.get("id").unwrap();

        response_html(fragment::post(id, "Foo", "Content"))
    }
}

fn response_html(html: maud::PreEscaped<String>) -> hyper::Response<Body> {
    ResponseBuilder::new()
        .header("Content-Type", "text/html")
        .status(StatusCode::OK)
        .body(Body::new(html.into_string()))
        .unwrap()
}

fn response_html_not_found(html: maud::PreEscaped<String>) -> hyper::Response<Body> {
    ResponseBuilder::new()
        .header("Content-Type", "text/html")
        .status(StatusCode::NOT_FOUND)
        .body(Body::new(html.into_string()))
        .unwrap()
}

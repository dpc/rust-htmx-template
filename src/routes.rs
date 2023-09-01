use std::sync::atomic::Ordering;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use hyper::{Response, StatusCode};
use maud::{html, Markup};

use crate::fragment::ResponseBuilderExt;
use crate::{fragment, Service};

pub async fn count(State(service): State<Service>) -> Markup {
    let count = service.state.count.fetch_add(1, Ordering::Relaxed) + 1;

    html! {
        (count)
    }
}

pub async fn home(State(service): State<Service>) -> Markup {
    fragment::page(
        "home",
        html! {
            article {
                h2 { "An htmx button" }
                p {
                    button name="foo" hx-post="/count" hx-swap="innerHTML" {
                        (service.state.count.load(Ordering::Relaxed))
                    }
                }
            }

            (fragment::post("post-123", "A blogpost", "Lorem ipsum, something something."))
        },
    )
}

pub async fn not_found_404() -> Markup {
    fragment::page(
        "PAGE NOT FOUND",
        html! {
            h2 { "This page does not exist. Sorry!" }
            p {
                a href="/" { "Return to the main page" }
            }
        },
    )
}

pub async fn too_many_requests_429() -> impl IntoResponse {
    Response::builder()
        .cache_nostore()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .body_static_str("text/plain", "Too Many Requests")
}

pub async fn favicon_ico() -> impl IntoResponse {
    Response::builder()
        .cache_static()
        .body_static_bytes("image/gif", include_bytes!("../static/dpc.gif").as_slice())
}

pub async fn style_css() -> impl IntoResponse {
    Response::builder()
        // .cache_static()
        .body_static_str("text/css", include_str!("../static/style.css"))
}

/// GET '/user/:id'
pub async fn get_user(Path(user_id): Path<u64>) -> Markup {
    html! { p { "User #"(user_id)  } }
}

pub async fn edit_post(Path(id): Path<String>) -> Markup {
    fragment::post_edit_form(&id, "Foo", "Content")
}

pub async fn save_post(Path(id): Path<String>) -> Markup {
    fragment::post(&id, "Foo", "Content")
}

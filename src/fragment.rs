use axum::body::Full;
use axum::response::IntoResponse;
use hyper::http::response;
use hyper::StatusCode;
use maud::{html, Markup, DOCTYPE};

pub fn page(title: &str, content: Markup) -> Markup {
    /// A basic header with a dynamic `page_title`.
    pub(crate) fn head(page_title: &str) -> Markup {
        html! {
            (DOCTYPE)
            html lang="en";
            head {
                meta charset="utf-8";
                link rel="stylesheet" type="text/css" href="/style.css";
                title { "dpc - " (page_title) }
            }
        }
    }

    pub(crate) fn header() -> Markup {
        html! {
            header {
                .content.split {
                    nav .column .text-column {
                        a href="/" { "Home" }
                        a href="/" { "Home2" }
                    }
                    .column .img-column {
                        img src="/favicon.ico" style="image-rendering: pixelated;" alt="dpc's avatar image";
                    }
                 }
            }
        }
    }

    /// A static footer.
    pub(crate) fn footer() -> Markup {
        html! {
            footer {
                .content.split {
                    h3 {
                        "Dawid Ciężarkiewicz"
                        br;
                        span.subtitle { "aka " span.dpc { "dpc" } }
                    }
                    p {
                        "Copyleft "
                        a href="https://dpc.pw" { "dpc" }
                        " with "
                        a href="https://x.dpc.pw" { "unclicked link" }
                    }
                }
            }
            script src="https://unpkg.com/htmx.org@1.9.4" {};
        }
    }

    html! {
        (head(title))
        body {
            (header())
            main.content {
                (content)
            }
            (footer())
        }
    }
}

pub(crate) fn post(id: &str, title: &str, body: &str) -> Markup {
    html! {
        article .post #id {
            h2 { (title) }

            p {
                (body)
            }

            button hx-get={ "/post/"(id)"/edit" } hx-swap="outerHTML" hx-target={ "closest article" } { "Edit" }
        }
    }
}

pub(crate) fn post_edit_form(id: &str, title: &str, body: &str) -> Markup {
    html! {
        article .post #id {
            form {
                input type="text" value=(title);
                textarea wrap="soft" { (body) }
                button hx-post={ "/post/"(id) } hx-swap="outerHTML" hx-target={ "closest article" } { "Submit" }
            }
        }
    }
}

pub trait ResponseBuilderExt {
    fn cache_static(self) -> Self;
    fn cache_nostore(self) -> Self;
    fn status_not_found(self) -> Self;

    // fn body_html(self, html: maud::PreEscaped<String>) -> Self::Response;
    fn body_static_str(self, content_type: &str, content: &'static str)
        -> axum::response::Response;
    fn body_static_bytes(
        self,
        content_type: &str,
        content: &'static [u8],
    ) -> axum::response::Response;
}

impl ResponseBuilderExt for response::Builder {
    fn cache_static(self) -> Self {
        self.header(
            "Cache-Control",
            "max-age=86400, stale-while-revalidate=86400",
        )
    }
    fn cache_nostore(self) -> Self {
        self.header("Cache-Control", "nostore")
    }

    fn status_not_found(self) -> Self {
        self.status(StatusCode::NOT_FOUND)
    }

    // fn body_html(self, html: maud::PreEscaped<String>) -> Self::Response {
    //     // self.header("Content-Type", "text/html")
    //     //     .body(astra::Body::new(html.into_string()))
    //     //     .unwrap()

    //     html.into_string().into()
    // }

    fn body_static_str(
        self,
        content_type: &str,
        content: &'static str,
    ) -> axum::response::Response {
        self.header("Content-Type", content_type)
            .body(Full::from(content))
            .unwrap()
            .into_response()
    }
    fn body_static_bytes(
        self,
        content_type: &str,
        content: &'static [u8],
    ) -> axum::response::Response {
        self.header("Content-Type", content_type)
            .body(Full::from(content))
            .unwrap()
            .into_response()
    }
}

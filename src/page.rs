use astra::ResponseBuilder;
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
            section.content {
                (content)
            }
            (footer())
        }
    }
}

pub trait ResponseBuilderExt {
    fn cache_static(self) -> Self;
}

impl ResponseBuilderExt for ResponseBuilder {
    fn cache_static(self) -> Self {
        self.header(
            "Cache-Control",
            "max-age=86400, stale-while-revalidate=86400",
        )
    }
}

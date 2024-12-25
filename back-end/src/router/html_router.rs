use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{event, Level};

pub(crate) fn build_html_router() -> Router {
    if cfg!(debug_assertions) {
        event!(Level::INFO, "Serving website via proxy");
        // TODO we'll want to be able to pass this in as an ENV variable
        let vite_proxy_service_builder =
            reverse_proxy_service::builder_http("127.0.0.1:4000").unwrap();

        let svc: reverse_proxy_service::ReusedService<
            reverse_proxy_service::Identity,
            reverse_proxy_service::client::HttpConnector,
            _,
        > = vite_proxy_service_builder.build(reverse_proxy_service::rewrite::Identity {});

        Router::new().nest_service("/", svc)
    } else {
        event!(Level::INFO, "Serving website from dist");
        Router::new().nest_service(
            "/",
            ServeDir::new("dist").fallback(ServeFile::new("dist/index.html")),
        )
    }
}

#![cfg(feature = "axum")]

use std::sync::Arc;

use axum::{
    body::HttpBody, extract::Path, http::StatusCode, response::IntoResponse, routing, Extension,
    Json, Router,
};

use crate::{Config, SwaggerUi, Url};

impl<B> From<SwaggerUi> for Router<(), B>
where
    B: HttpBody + Send + 'static,
{
    fn from(swagger_ui: SwaggerUi) -> Self {
        let urls_capacity = swagger_ui.urls.len();

        let (router, urls) = swagger_ui.urls.into_iter().fold(
            (
                Router::<(), B>::new(),
                Vec::<Url>::with_capacity(urls_capacity),
            ),
            |(router, mut urls), url| {
                let (url, openapi) = url;
                (
                    router.route(
                        url.url.as_ref(),
                        routing::get(move || async { Json(openapi) }),
                    ),
                    {
                        urls.push(url);
                        urls
                    },
                )
            },
        );

        let config = if let Some(config) = swagger_ui.config {
            config.configure_defaults(urls)
        } else {
            Config::new(urls)
        };

        let path_no_slash = swagger_ui.path.trim_end_matches('/');

        router
            .route(
                &format!("{path_no_slash}/*tail"),
                routing::get(serve_swagger_ui),
            )
            .route(path_no_slash, routing::get(serve_swagger_ui))
            .route(&format!("{path_no_slash}/"), routing::get(serve_swagger_ui))
            .layer(Extension(Arc::new(config)))
    }
}

async fn serve_swagger_ui(
    tail: Option<Path<String>>,
    Extension(state): Extension<Arc<Config<'static>>>,
) -> impl IntoResponse {
    let sub_path = match &tail {
        None => "",
        Some(tail) => tail.as_str(),
    };

    match super::serve(sub_path, state) {
        Ok(file) => file
            .map(|file| {
                (
                    StatusCode::OK,
                    [("Content-Type", file.content_type)],
                    file.bytes,
                )
                    .into_response()
            })
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response()),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

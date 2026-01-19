use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use aish_client::HttpTransport;
use aish_client::StreamResponse;
use aish_client::run_with_retry;
use http::HeaderMap;
use http::Method;
use serde_json::Value;
use std::time::Duration;

pub(crate) struct StreamingClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
}

impl<T: HttpTransport, A: AuthProvider> StreamingClient<T, A> {
    pub(crate) fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
        }
    }

    pub(crate) fn provider(&self) -> &Provider {
        &self.provider
    }

    pub(crate) async fn stream(
        &self,
        path: &str,
        body: Value,
        extra_headers: HeaderMap,
        spawner: fn(StreamResponse, Duration) -> ResponseStream,
    ) -> Result<ResponseStream, ApiError> {
        let builder = || {
            let mut req = self.provider.build_request(Method::POST, path);
            req.headers.extend(extra_headers.clone());
            req.headers.insert(
                http::header::ACCEPT,
                http::HeaderValue::from_static("text/event-stream"),
            );
            req.body = Some(body.clone());
            add_auth_headers(&self.auth, req)
        };

        let stream_response = run_with_retry(self.provider.retry.to_policy(), builder, |req| {
            self.transport.stream(req)
        })
        .await?;

        Ok(spawner(stream_response, self.provider.stream_idle_timeout))
    }
}

use aish_api::AuthProvider as ApiAuthProvider;
use aish_api::TransportError;
use aish_api::error::ApiError;
use chrono::DateTime;
use chrono::Utc;
use http::HeaderMap;
use serde::Deserialize;

use crate::auth::AishAuth;
use crate::error::AishErr;
use crate::error::RetryLimitReachedError;
use crate::error::UnexpectedResponseError;
use crate::error::UsageLimitReachedError;
use crate::model_provider_info::ModelProviderInfo;
use crate::token_data::PlanType;

pub(crate) fn map_api_error(err: ApiError) -> AishErr {
    match err {
        ApiError::ContextWindowExceeded => AishErr::ContextWindowExceeded,
        ApiError::QuotaExceeded => AishErr::QuotaExceeded,
        ApiError::UsageNotIncluded => AishErr::UsageNotIncluded,
        ApiError::Retryable { message, delay } => AishErr::Stream(message, delay),
        ApiError::Stream(msg) => AishErr::Stream(msg, None),
        ApiError::Api { status, message } => AishErr::UnexpectedStatus(UnexpectedResponseError {
            status,
            body: message,
            request_id: None,
        }),
        ApiError::Transport(transport) => match transport {
            TransportError::Http {
                status,
                headers,
                body,
            } => {
                let body_text = body.unwrap_or_default();

                if status == http::StatusCode::BAD_REQUEST {
                    if body_text
                        .contains("The image data you provided does not represent a valid image")
                    {
                        AishErr::InvalidImageRequest()
                    } else {
                        AishErr::InvalidRequest(body_text)
                    }
                } else if status == http::StatusCode::INTERNAL_SERVER_ERROR {
                    AishErr::InternalServerError
                } else if status == http::StatusCode::TOO_MANY_REQUESTS {
                    if let Ok(err) = serde_json::from_str::<UsageErrorResponse>(&body_text) {
                        if err.error.error_type.as_deref() == Some("usage_limit_reached") {
                            let resets_at = err
                                .error
                                .resets_at
                                .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0));
                            return AishErr::UsageLimitReached(UsageLimitReachedError {
                                plan_type: err.error.plan_type,
                                resets_at,
                            });
                        } else if err.error.error_type.as_deref() == Some("usage_not_included") {
                            return AishErr::UsageNotIncluded;
                        }
                    }

                    AishErr::RetryLimit(RetryLimitReachedError {
                        status,
                        request_id: extract_request_id(headers.as_ref()),
                    })
                } else {
                    AishErr::UnexpectedStatus(UnexpectedResponseError {
                        status,
                        body: body_text,
                        request_id: extract_request_id(headers.as_ref()),
                    })
                }
            }
            TransportError::RetryLimit => AishErr::RetryLimit(RetryLimitReachedError {
                status: http::StatusCode::INTERNAL_SERVER_ERROR,
                request_id: None,
            }),
            TransportError::Timeout => AishErr::Timeout,
            TransportError::Network(msg) | TransportError::Build(msg) => AishErr::Stream(msg, None),
        },
    }
}

fn extract_request_id(headers: Option<&HeaderMap>) -> Option<String> {
    headers.and_then(|map| {
        ["cf-ray", "x-request-id", "x-oai-request-id"]
            .iter()
            .find_map(|name| {
                map.get(*name)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string)
            })
    })
}

pub(crate) async fn auth_provider_from_auth(
    auth: Option<AishAuth>,
    provider: &ModelProviderInfo,
) -> crate::error::Result<CoreAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(CoreAuthProvider {
            token: Some(api_key),
            account_id: None,
        });
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(CoreAuthProvider {
            token: Some(token),
            account_id: None,
        });
    }

    if let Some(auth) = auth {
        let token = auth.get_token().await?;
        Ok(CoreAuthProvider {
            token: Some(token),
            account_id: auth.get_account_id(),
        })
    } else {
        Ok(CoreAuthProvider {
            token: None,
            account_id: None,
        })
    }
}

#[derive(Debug, Deserialize)]
struct UsageErrorResponse {
    error: UsageErrorBody,
}

#[derive(Debug, Deserialize)]
struct UsageErrorBody {
    #[serde(rename = "type")]
    error_type: Option<String>,
    plan_type: Option<PlanType>,
    resets_at: Option<i64>,
}

#[derive(Clone, Default)]
pub(crate) struct CoreAuthProvider {
    token: Option<String>,
    account_id: Option<String>,
}

impl ApiAuthProvider for CoreAuthProvider {
    fn bearer_token(&self) -> Option<String> {
        self.token.clone()
    }

    fn account_id(&self) -> Option<String> {
        self.account_id.clone()
    }
}

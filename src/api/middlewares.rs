use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::{ErrorInternalServerError, ErrorUnauthorized},
    Error,
};
use actix_web_lab::middleware::{from_fn, Next};
use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};

pub fn metrics() -> PrometheusMetrics {
    PrometheusMetricsBuilder::new("dcl_camera_reel_service")
        .endpoint("/metrics")
        .build()
        .unwrap()
}

fn validate_token(bearer_token: String, request: &ServiceRequest) -> Result<(), Error> {
    let path = request.path();
    if path == "/metrics" {
        let token = request
            .headers()
            .get("Authorization")
            .map(|token| token.to_str());
        if bearer_token.is_empty() {
            tracing::error!("missing wkc_metrics_bearer_token in configuration");
            return Err(ErrorInternalServerError(""));
        }

        match token {
            Some(Ok(token)) => {
                if token.len() > 7 {
                    let mut parts = token.splitn(2, ' ');
                    match parts.next() {
                        Some(scheme) if scheme == "Bearer" => {}
                        _ => return Err(ErrorUnauthorized("Wrong schema")),
                    }
                    if let Some(token) = parts.next() {
                        if token != bearer_token {
                            return Err(ErrorUnauthorized("Invalid token"));
                        }
                    } else {
                        return Err(ErrorUnauthorized("Missing token"));
                    }
                } else {
                    tracing::error!("invalid bearer token for /metrics");
                    return Err(ErrorUnauthorized("Missing token"));
                }
            }
            _ => {
                return Err(ErrorUnauthorized("Token not present"));
            }
        }
    }

    Ok(())
}

pub fn metrics_token<S, B>(
    bearer_token: &str,
) -> impl Transform<
    S,
    ServiceRequest,
    Response = ServiceResponse<impl MessageBody>,
    Error = Error,
    InitError = (),
>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    let token = bearer_token.to_owned();
    from_fn(move |req, next: Next<B>| {
        let token = token.clone();
        async move {
            validate_token(token, &req)?;
            next.call(req).await
        }
    })
}

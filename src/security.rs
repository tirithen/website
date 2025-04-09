use axum::{
    body::Body,
    http::{HeaderValue, Request, Response, header},
    middleware::Next,
};

pub async fn add_security_headers(request: Request<Body>, next: Next) -> Response<Body> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    let security_headers = [
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; frame-ancestors 'none'; form-action 'self'; base-uri 'self';",
        ),
        (
            header::STRICT_TRANSPORT_SECURITY,
            "max-age=31536000; includeSubDomains",
        ),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (
            header::HeaderName::from_static("permissions-policy"),
            "interest-cohort=(), accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), display-capture=(), document-domain=(), encrypted-media=(), execution-while-not-rendered=(), execution-while-out-of-viewport=(), fullscreen=(), geolocation=(), gyroscope=(), keyboard-map=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=(), picture-in-picture=(), publickey-credentials-get=(), screen-wake-lock=(), sync-xhr=(), usb=(), web-share=(), xr-spatial-tracking=()",
        ),
        (header::REFERRER_POLICY, "no-referrer"),
        (
            header::HeaderName::from_static("cross-origin-opener-policy"),
            "same-origin",
        ),
        (
            header::HeaderName::from_static("cross-origin-opener-policy"),
            "require-corp",
        ),
        (
            header::HeaderName::from_static("cross-origin-resource-policy"),
            "same-origin",
        ),
    ];

    for (name, value) in security_headers {
        headers.insert(name, HeaderValue::from_static(value));
    }

    response
}

use std::{fmt, sync::Arc};

/// HTTP request encapsulation.
///
/// This type represents an HTTP request that can be sent to the Anthropic API.
#[derive(Debug)]
pub struct HttpRequest {
    /// Request host.
    pub host: String,
    /// Request path.
    pub path: String,
    /// HTTP method.
    pub method: &'static str,
    /// Request headers.
    pub headers: Vec<(&'static str, Arc<str>)>,
    /// Request body.
    pub body: String,
}

impl HttpRequest {
    pub fn render_headers(&self) -> String {
        self.headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write request line
        writeln!(f, "{} {} HTTP/1.1", self.method, self.path)?;

        // Write Host header first
        writeln!(f, "Host: {}", self.host)?;

        // Write other headers
        for (key, value) in &self.headers {
            writeln!(f, "{}: {}", key, value.as_ref())?;
        }

        // Empty line between headers and body
        writeln!(f)?;

        // Write body
        write!(f, "{}", self.body)?;

        Ok(())
    }
}

#[cfg(feature = "reqwest")]
impl HttpRequest {
    /// Converts this HttpRequest into a reqwest::Request.
    pub fn try_into_reqwest(self) -> Result<reqwest::Request, Box<dyn std::error::Error>> {
        let method = reqwest::Method::from_bytes(self.method.as_bytes())?;

        let url_string = format!("https://{}{}", self.host, self.path);
        let url = reqwest::Url::parse(&url_string)?;
        let mut request = reqwest::Request::new(method, url);

        // Set body
        *request.body_mut() = Some(self.body.into());

        // Add headers
        let headers = request.headers_mut();
        for (key, value) in self.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
            let header_value = reqwest::header::HeaderValue::from_str(&value)?;
            headers.insert(header_name, header_value);
        }

        Ok(request)
    }
}

#[cfg(feature = "reqwest-blocking")]
impl HttpRequest {
    /// Converts this HttpRequest into a reqwest::blocking::Request.
    pub fn try_into_reqwest_blocking(
        self,
    ) -> Result<reqwest::blocking::Request, Box<dyn std::error::Error>> {
        let method = reqwest::Method::from_bytes(self.method.as_bytes())?;

        let url_string = format!("https://{}{}", self.host, self.path);
        let url = reqwest::Url::parse(&url_string)?;
        let mut request = reqwest::blocking::Request::new(method, url);

        // Set body
        *request.body_mut() = Some(self.body.into());

        // Add headers
        let headers = request.headers_mut();
        for (key, value) in self.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
            let header_value = reqwest::header::HeaderValue::from_str(&value)?;
            headers.insert(header_name, header_value);
        }

        Ok(request)
    }
}

#[cfg(feature = "reqwest")]
impl From<HttpRequest> for reqwest::Request {
    fn from(http_request: HttpRequest) -> Self {
        http_request
            .try_into_reqwest()
            .expect("failed to convert to reqwest::Request")
    }
}

#[cfg(feature = "reqwest-blocking")]
impl From<HttpRequest> for reqwest::blocking::Request {
    fn from(http_request: HttpRequest) -> Self {
        http_request
            .try_into_reqwest_blocking()
            .expect("failed to convert to reqwest::blocking::Request")
    }
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_http_request_to_reqwest_conversion() {
        let http_request = super::HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", std::sync::Arc::from("application/json")),
                ("anthropic-version", std::sync::Arc::from("2023-06-01")),
                ("x-api-key", std::sync::Arc::from("test-key")),
                (
                    "anthropic-model",
                    std::sync::Arc::from("claude-3-sonnet-20240229"),
                ),
                ("max-tokens", std::sync::Arc::from("1024")),
            ],
            body:
                r#"{"messages":[{"role":"user","content":{"type":"text","text":"Hello, world!"}}]}"#
                    .to_string(),
        };

        // Convert to reqwest::Request
        let reqwest_request: reqwest::Request = http_request
            .try_into()
            .expect("should convert successfully");

        assert_eq!(reqwest_request.method(), &reqwest::Method::POST);
        assert_eq!(
            reqwest_request.url().as_str(),
            "https://api.anthropic.com/v1/messages"
        );

        let headers = reqwest_request.headers();
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");

        let body = reqwest_request.body().unwrap();
        let body_bytes = body.as_bytes().unwrap();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("Hello, world!"));
        assert!(body_str.contains("\"type\":\"text\""));
    }

    #[cfg(feature = "reqwest-blocking")]
    #[test]
    fn test_http_request_to_reqwest_blocking_conversion() {
        let http_request = super::HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", std::sync::Arc::from("application/json")),
                ("anthropic-version", std::sync::Arc::from("2023-06-01")),
                ("x-api-key", std::sync::Arc::from("test-key")),
                (
                    "anthropic-model",
                    std::sync::Arc::from("claude-3-sonnet-20240229"),
                ),
                ("max-tokens", std::sync::Arc::from("1024")),
            ],
            body:
                r#"{"messages":[{"role":"user","content":{"type":"text","text":"Hello, world!"}}]}"#
                    .to_string(),
        };

        // Convert to reqwest::blocking::Request
        let reqwest_request: reqwest::blocking::Request = http_request
            .try_into()
            .expect("should convert successfully");

        assert_eq!(reqwest_request.method(), &reqwest::Method::POST);
        assert_eq!(
            reqwest_request.url().as_str(),
            "https://api.anthropic.com/v1/messages"
        );

        let headers = reqwest_request.headers();
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");

        let body = reqwest_request.body().unwrap();
        let body_bytes = body.as_bytes().unwrap();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("Hello, world!"));
        assert!(body_str.contains("\"type\":\"text\""));
    }
}

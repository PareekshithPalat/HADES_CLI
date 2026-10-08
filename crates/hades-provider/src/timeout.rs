use hades_config::ProviderConfig;
use std::time::Duration;

/// Timeout used for lightweight metadata calls (authentication probe, model discovery).
const METADATA_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Network timeout policy applied by a provider adapter.
///
/// `None` disables the corresponding timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderTimeouts {
    /// Maximum time allowed to establish the TCP/TLS connection.
    pub connect: Duration,
    /// Total time allowed for non-streaming completion requests.
    pub request: Option<Duration>,
    /// Maximum silence allowed between streamed chunks (including time-to-first-token).
    pub stream_idle: Option<Duration>,
    /// Total time allowed for metadata calls such as `/models`.
    pub metadata: Duration,
}

fn secs(value: u64) -> Option<Duration> {
    (value > 0).then(|| Duration::from_secs(value))
}

impl ProviderTimeouts {
    /// Resolves the timeout policy for a provider from user configuration.
    pub fn from_config(config: &ProviderConfig, is_local: bool) -> Self {
        let (request, stream_idle) = if is_local {
            (
                config.local_request_timeout_secs,
                config.local_stream_idle_timeout_secs,
            )
        } else {
            (
                config.cloud_request_timeout_secs,
                config.cloud_stream_idle_timeout_secs,
            )
        };

        Self {
            connect: Duration::from_secs(config.connect_timeout_secs.max(1)),
            request: secs(request),
            stream_idle: secs(stream_idle),
            metadata: METADATA_REQUEST_TIMEOUT,
        }
    }

    /// Default timeout policy for local (`is_local = true`) or cloud providers.
    pub fn defaults(is_local: bool) -> Self {
        Self::from_config(&ProviderConfig::default(), is_local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_defaults_have_no_total_timeout() {
        let t = ProviderTimeouts::defaults(true);
        assert_eq!(t.request, None);
        assert!(t.stream_idle.unwrap() > Duration::from_secs(60));
        assert!(t.connect <= Duration::from_secs(10));
    }

    #[test]
    fn test_cloud_defaults_keep_bounded_timeouts() {
        let t = ProviderTimeouts::defaults(false);
        assert!(t.request.is_some());
        assert!(t.stream_idle.is_some());
    }

    #[test]
    fn test_zero_disables_timeout() {
        let config = ProviderConfig {
            local_stream_idle_timeout_secs: 0,
            cloud_request_timeout_secs: 0,
            ..Default::default()
        };
        assert_eq!(
            ProviderTimeouts::from_config(&config, true).stream_idle,
            None
        );
        assert_eq!(ProviderTimeouts::from_config(&config, false).request, None);
    }
}

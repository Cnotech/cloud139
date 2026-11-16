//! 请求级重试：**只重试瞬时错误**（网络断连、IO、超时），不重试业务错误。
//!
//! 为什么要分：把「文件不存在」也重试 4 次、每次退避 2/4/8/16 秒，一个缺失对象就要白等 30 秒；
//! 而断连这类瞬时错误恰恰是重试能救回来的。
//!
//! 分工（避免叠成 5×5）：
//! - 本模块挂在**调用点**（列表、下载、路径解析等读操作）；
//! - 底层 HTTP 客户端**不加**重试；
//! - 所以一次失败最多重试 `retries` 次，不会出现「上层 5 × 下层 5」。

use std::time::Duration;

use crate::client::ClientError;
use crate::warn;

/// 指数退避：base × 2^attempt，attempt 上限 10 次方（避免溢出）。
#[must_use]
pub fn backoff_delay(attempt: u32, base_secs: u64) -> Duration {
    if base_secs == 0 {
        return Duration::from_millis(0);
    }
    let factor = 2u64.saturating_pow(attempt.min(10));
    Duration::from_secs(base_secs.saturating_mul(factor))
}

/// 是否是「重试有意义」的错误：网络/IO/超时。
///
/// 业务错误（FileNotFound、Api 返回的业务码、未登录、配置错）重试不会变好，直接返回。
#[must_use]
pub fn is_transient(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(client_err) = cause.downcast_ref::<ClientError>() {
            return match client_err {
                ClientError::Http(err) => http_is_transient(err),
                ClientError::Io(io) => io_is_transient(io.kind()),
                _ => false,
            };
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return io_is_transient(io.kind());
        }
    }
    false
}

/// reqwest 错误里只有连接、超时、传输、半包值得重试。
///
/// Builder（请求构造）与 Decode（响应 JSON 与模型不匹配）是确定性的，重试不会变好；
/// `resp.json()` 的模型解码错误也会走 `ClientError::Http`，不能一概按瞬时分。
fn http_is_transient(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request() || err.is_body()
}

/// IO 错误里哪些值得重试：断连、超时、半包。NotFound / PermissionDenied 重试也不会好。
fn io_is_transient(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::Interrupted
    )
}

/// 就地重试：最多 `retries` 次（首次 + retries 次重试），每次退避 `base_secs × 2^n`。
///
/// 只重试瞬时错误；耗尽后把错误返回，由调用方决定退出码。
pub async fn with_retries<T, F, Fut>(
    retries: u32,
    base_secs: u64,
    mut action: F,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        match action().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if attempt >= retries || !is_transient(&err) {
                    if attempt == 0 {
                        return Err(err);
                    }
                    return Err(err.context(format!("重试 {attempt} 次后仍失败")));
                }
                let delay = backoff_delay(attempt, base_secs);
                warn!("操作失败，{} 秒后重试: {err:#}", delay.as_secs());
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_exponentially() {
        assert_eq!(backoff_delay(0, 3).as_secs(), 3);
        assert_eq!(backoff_delay(1, 3).as_secs(), 6);
        assert_eq!(backoff_delay(2, 3).as_secs(), 12);
        assert_eq!(backoff_delay(3, 3).as_secs(), 24);
    }

    #[test]
    fn zero_base_means_no_wait() {
        assert_eq!(backoff_delay(2, 0).as_secs(), 0);
    }

    #[test]
    fn business_errors_are_not_transient() {
        let err: anyhow::Error = ClientError::FileNotFound.into();
        assert!(!is_transient(&err));

        let err: anyhow::Error = ClientError::Api("文件或目录不存在".to_string()).into();
        assert!(!is_transient(&err));

        let err: anyhow::Error = ClientError::NotLoggedIn.into();
        assert!(!is_transient(&err));
    }

    #[test]
    fn io_errors_are_transient() {
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        let err: anyhow::Error = ClientError::Io(io).into();
        assert!(is_transient(&err));
    }

    /// 本地 IO 的断连/超时也应该重试；但「文件不存在」「没权限」不该重试。
    #[test]
    fn plain_io_errors_are_classified() {
        let transient: anyhow::Error =
            std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout").into();
        assert!(is_transient(&transient));

        let not_found: anyhow::Error =
            std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
        assert!(!is_transient(&not_found));

        let denied: anyhow::Error =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied").into();
        assert!(!is_transient(&denied));
    }

    /// 业务错误一次就返回，不浪费时间重试。
    #[tokio::test]
    async fn business_error_is_not_retried() {
        let mut calls = 0;
        let result: anyhow::Result<()> = with_retries(3, 0, || {
            calls += 1;
            async { Err(ClientError::FileNotFound.into()) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1, "业务错误不应重试");
    }

    /// 瞬时错误按次数重试，耗尽后带上尝试次数。
    #[tokio::test]
    async fn transient_error_is_retried_until_exhausted() {
        let mut calls = 0;
        let result: anyhow::Result<()> = with_retries(2, 0, || {
            calls += 1;
            async {
                Err::<(), anyhow::Error>(
                    ClientError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "reset",
                    ))
                    .into(),
                )
            }
        })
        .await;
        let err = result.unwrap_err();
        assert_eq!(calls, 3, "1 次首次 + 2 次重试");
        assert!(
            format!("{err:#}").contains("重试 2 次后仍失败"),
            "错误信息要带尝试次数: {err:#}"
        );
    }

    /// 瞬时错误只要有一次成功就返回成功。
    #[tokio::test]
    async fn transient_error_recovers() {
        let mut calls = 0;
        let result: anyhow::Result<u32> = with_retries(3, 0, || {
            calls += 1;
            let current = calls;
            async move {
                if current < 2 {
                    Err(ClientError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "reset",
                    ))
                    .into())
                } else {
                    Ok(current)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 2);
        assert_eq!(calls, 2);
    }

    /// 实际只重试了 1 次就遇到不可重试错误时，报的是 1，不是配置上限。
    #[tokio::test]
    async fn reports_actual_retry_count_when_error_is_no_longer_transient() {
        let mut calls = 0;
        let result: anyhow::Result<()> = with_retries(4, 0, || {
            calls += 1;
            let current = calls;
            async move {
                if current == 1 {
                    Err(ClientError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "reset",
                    ))
                    .into())
                } else {
                    Err(ClientError::FileNotFound.into())
                }
            }
        })
        .await;
        let err = result.unwrap_err();
        assert_eq!(calls, 2, "首次断连 + 第二次业务错误");
        assert!(
            format!("{err:#}").contains("重试 1 次后仍失败"),
            "应报实际重试次数: {err:#}"
        );
    }
}

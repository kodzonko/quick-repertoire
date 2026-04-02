use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use log::debug;
use scraper::{Html, Selector};
use tokio::time::{Instant, sleep};

use crate::error::{AppError, AppResult};
use crate::retry::{RetryDirective, RetryPolicy, retry_with_backoff};

const REQUEST_TIMEOUT_SECONDS: u64 = 30;
const HTML_POLL_INTERVAL_MILLIS: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserEvaluation {
    pub name: String,
    pub expression: String,
}

impl BrowserEvaluation {
    pub fn new(name: impl Into<String>, expression: impl Into<String>) -> Self {
        Self { name: name.into(), expression: expression.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedPage {
    pub html: String,
    pub evaluations: HashMap<String, String>,
}

impl RenderedPage {
    pub fn evaluation(&self, name: &str) -> Option<&str> {
        self.evaluations.get(name).map(String::as_str)
    }
}

#[async_trait]
pub trait HtmlRenderer: Send + Sync {
    async fn render(&self, url: &str, wait_selector: &str) -> AppResult<String>;

    async fn render_with_evaluations(
        &self,
        url: &str,
        wait_selector: &str,
        _evaluations: &[BrowserEvaluation],
    ) -> AppResult<RenderedPage> {
        Ok(RenderedPage {
            html: self.render(url, wait_selector).await?,
            evaluations: HashMap::new(),
        })
    }
}

#[async_trait]
trait RenderedHtmlSource: Send + Sync {
    async fn content(&self) -> AppResult<String>;
}

#[async_trait]
impl RenderedHtmlSource for Page {
    async fn content(&self) -> AppResult<String> {
        Page::content(self).await.map_err(|error| AppError::BrowserUnavailable(error.to_string()))
    }
}

#[derive(Clone)]
pub struct ChromiumHtmlRenderer;

#[async_trait]
impl HtmlRenderer for ChromiumHtmlRenderer {
    async fn render(&self, url: &str, wait_selector: &str) -> AppResult<String> {
        Ok(self.render_with_evaluations(url, wait_selector, &[]).await?.html)
    }

    async fn render_with_evaluations(
        &self,
        url: &str,
        wait_selector: &str,
        evaluations: &[BrowserEvaluation],
    ) -> AppResult<RenderedPage> {
        debug!(
            "Chromium render starting url={url} wait_selector={wait_selector} timeout_secs={REQUEST_TIMEOUT_SECONDS} evaluations={}",
            evaluations.len(),
        );
        let (mut browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .request_timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
                .build()
                .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?,
        )
        .await
        .map_err(|error| {
            AppError::BrowserUnavailable(format!(
                "Nie udało się uruchomić przeglądarki Chromium: {error}"
            ))
        })?;

        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                let _ = event;
            }
        });

        let page = browser
            .new_page(url)
            .await
            .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?;
        let html = wait_for_selector_in_rendered_html(
            &page,
            url,
            wait_selector,
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            Duration::from_millis(HTML_POLL_INTERVAL_MILLIS),
        )
        .await?;

        let mut evaluation_results = HashMap::new();
        for evaluation in evaluations {
            let value = page
                .evaluate(evaluation.expression.as_str())
                .await
                .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?
                .into_value::<String>()
                .map_err(|error| AppError::BrowserUnavailable(error.to_string()))?;
            evaluation_results.insert(evaluation.name.clone(), value);
        }

        close_browser_session(&mut browser).await;
        handler_task.abort();

        Ok(RenderedPage { html, evaluations: evaluation_results })
    }
}

#[async_trait]
trait BrowserSession: Send {
    async fn request_close(&mut self);
    async fn wait_for_exit(&mut self);
}

#[async_trait]
impl BrowserSession for Browser {
    async fn request_close(&mut self) {
        let _ = self.close().await;
    }

    async fn wait_for_exit(&mut self) {
        let _ = self.wait().await;
    }
}

async fn close_browser_session(browser: &mut dyn BrowserSession) {
    browser.request_close().await;
    browser.wait_for_exit().await;
}

pub async fn render_html_with_retry(
    renderer: &dyn HtmlRenderer,
    retry_policy: RetryPolicy,
    url: &str,
    wait_selector: &str,
) -> AppResult<String> {
    Ok(render_page_with_retry(renderer, retry_policy, url, wait_selector, &[]).await?.html)
}

pub async fn render_page_with_retry(
    renderer: &dyn HtmlRenderer,
    retry_policy: RetryPolicy,
    url: &str,
    wait_selector: &str,
    evaluations: &[BrowserEvaluation],
) -> AppResult<RenderedPage> {
    retry_with_backoff(retry_policy, |attempt| async move {
        debug!(
            "Browser render attempt={attempt} url={url} wait_selector={wait_selector} evaluations={}",
            evaluations.len(),
        );
        renderer
            .render_with_evaluations(url, wait_selector, evaluations)
            .await
            .map_err(|error| classify_render_error(attempt, url, wait_selector, error))
    })
    .await
}

fn classify_render_error(
    attempt: usize,
    url: &str,
    wait_selector: &str,
    error: AppError,
) -> RetryDirective<AppError> {
    match error {
        error @ AppError::BrowserUnavailable(_) => {
            debug!(
                "Browser render failed with a retryable browser error attempt={attempt} url={url} wait_selector={wait_selector} error={error}"
            );
            RetryDirective::retry(error)
        }
        error => {
            debug!(
                "Browser render failed with a non-retryable error attempt={attempt} url={url} wait_selector={wait_selector} error={error}"
            );
            RetryDirective::fail(error)
        }
    }
}

async fn wait_for_selector_in_rendered_html(
    source: &dyn RenderedHtmlSource,
    url: &str,
    wait_selector: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> AppResult<String> {
    let selector = Selector::parse(wait_selector).expect("wait selector must compile");
    let deadline = Instant::now() + timeout;
    let mut last_transient_error = None;
    let mut is_first_attempt = true;
    let mut poll_attempt = 0_usize;

    loop {
        poll_attempt += 1;
        if !is_first_attempt && Instant::now() >= deadline {
            debug!(
                "Rendered HTML wait timed out url={url} wait_selector={wait_selector} attempts={poll_attempt} last_transient_error={last_transient_error:?}"
            );
            return Err(build_wait_timeout_error(
                url,
                wait_selector,
                last_transient_error.as_ref(),
            ));
        }
        is_first_attempt = false;

        match source.content().await {
            Ok(rendered_html) => {
                if Html::parse_document(&rendered_html).select(&selector).next().is_some() {
                    debug!(
                        "Rendered HTML selector found url={url} wait_selector={wait_selector} attempts={poll_attempt} html_bytes={}",
                        rendered_html.len(),
                    );
                    return Ok(rendered_html);
                }
            }
            Err(error) if is_transient_browser_error(&error) => {
                debug!(
                    "Rendered HTML source returned a transient browser error url={url} wait_selector={wait_selector} attempts={poll_attempt} error={error}"
                );
                last_transient_error = Some(error);
            }
            Err(error) => return Err(error),
        }

        if Instant::now() >= deadline {
            debug!(
                "Rendered HTML wait timed out url={url} wait_selector={wait_selector} attempts={poll_attempt} last_transient_error={last_transient_error:?}"
            );
            return Err(build_wait_timeout_error(
                url,
                wait_selector,
                last_transient_error.as_ref(),
            ));
        }

        sleep(poll_interval).await;
    }
}

fn is_transient_browser_error(error: &AppError) -> bool {
    match error {
        AppError::BrowserUnavailable(message) => {
            message.contains("Could not find node with given id")
                || message.contains("Cannot find context with specified id")
                || message.contains("Execution context was destroyed")
        }
        _ => false,
    }
}

fn build_wait_timeout_error(
    url: &str,
    wait_selector: &str,
    last_transient_error: Option<&AppError>,
) -> AppError {
    let base_message = format!(
        "Przekroczono limit czasu podczas oczekiwania na element `{wait_selector}` na stronie {url}."
    );
    match last_transient_error {
        Some(error) => AppError::BrowserUnavailable(format!(
            "{base_message} Ostatni błąd przeglądarki: {error}"
        )),
        None => AppError::BrowserUnavailable(base_message),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    struct FakeRenderedHtmlSource {
        responses: Mutex<VecDeque<AppResult<String>>>,
    }

    impl FakeRenderedHtmlSource {
        fn new(responses: Vec<AppResult<String>>) -> Self {
            Self { responses: Mutex::new(VecDeque::from(responses)) }
        }
    }

    #[async_trait]
    impl RenderedHtmlSource for FakeRenderedHtmlSource {
        async fn content(&self) -> AppResult<String> {
            self.responses
                .lock()
                .expect("rendered html responses lock poisoned")
                .pop_front()
                .expect("test response must be configured")
        }
    }

    #[tokio::test]
    async fn wait_for_selector_in_rendered_html_retries_transient_missing_node_errors() {
        let source = FakeRenderedHtmlSource::new(vec![
            Err(AppError::BrowserUnavailable(
                "Error -32000: Could not find node with given id".to_string(),
            )),
            Ok("<html><body><div>loading...</div></body></html>".to_string()),
            Ok("<html><body><div class=\"row qb-movie\">loaded</div></body></html>".to_string()),
        ]);

        let rendered_html = wait_for_selector_in_rendered_html(
            &source,
            "https://example.test/repertoire",
            "div.row.qb-movie",
            Duration::from_millis(50),
            Duration::from_millis(1),
        )
        .await
        .expect("transient node lookup errors should be retried");

        assert!(rendered_html.contains("qb-movie"));
    }

    #[tokio::test]
    async fn wait_for_selector_in_rendered_html_times_out_when_selector_never_appears() {
        let source = FakeRenderedHtmlSource::new(vec![
            Ok("<html><body><div>loading...</div></body></html>".to_string()),
            Ok("<html><body><div>still loading...</div></body></html>".to_string()),
        ]);

        let error = wait_for_selector_in_rendered_html(
            &source,
            "https://example.test/repertoire",
            "div.row.qb-movie",
            Duration::from_millis(2),
            Duration::from_millis(1),
        )
        .await
        .expect_err("missing selector should time out");

        assert_eq!(
            error,
            AppError::BrowserUnavailable(
                "Przekroczono limit czasu podczas oczekiwania na element `div.row.qb-movie` na stronie https://example.test/repertoire."
                    .to_string(),
            )
        );
    }

    struct FakeBrowserSession {
        calls: Vec<&'static str>,
    }

    #[async_trait]
    impl BrowserSession for FakeBrowserSession {
        async fn request_close(&mut self) {
            self.calls.push("close");
        }

        async fn wait_for_exit(&mut self) {
            self.calls.push("wait");
        }
    }

    #[tokio::test]
    async fn close_browser_session_waits_for_browser_exit_after_close_request() {
        let mut browser = FakeBrowserSession { calls: Vec::new() };

        close_browser_session(&mut browser).await;

        assert_eq!(browser.calls, vec!["close", "wait"]);
    }
}

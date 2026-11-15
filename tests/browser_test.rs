use std::time::Duration;

use httpmock::{Method::GET, MockServer};
use quick_repertoire::cinema::browser::{ChromiumHtmlRenderer, HtmlRenderer};

#[tokio::test]
#[ignore = "requires Chrome or Chromium"]
async fn renderer_runs_javascript_without_downloading_images() {
    let server = MockServer::start_async().await;
    let page = server
        .mock_async(|when, then| {
            when.method(GET).path("/");
            then.status(200).header("content-type", "text/html").body(
                r#"<body><img src="/poster.png"><script>
                document.body.insertAdjacentHTML('beforeend', '<h2 id="ready">Repertoire</h2>');
                </script></body>"#,
            );
        })
        .await;
    let image = server
        .mock_async(|when, then| {
            when.method(GET).path("/poster.png");
            then.status(200).delay(Duration::from_millis(500));
        })
        .await;

    let html = ChromiumHtmlRenderer.render(&server.url("/"), "#ready").await.unwrap();

    assert!(html.contains("<h2 id=\"ready\">Repertoire</h2>"));
    page.assert_async().await;
    image.assert_calls_async(0).await;
}

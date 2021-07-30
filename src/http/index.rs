use std::convert::Infallible;

const INDEX_TEMPLATE: &str = include_str!("index.html");

pub async fn serve_index(doc_path: String) -> Result<impl warp::Reply, Infallible> {
    let content = INDEX_TEMPLATE.replace("__url_path__", &doc_path);
    Ok(warp::reply::html(content))
}

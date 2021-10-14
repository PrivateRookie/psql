use std::convert::Infallible;

const INDEX_TEMPLATE: &str = include_str!("index.html");
const FAVICON: &[u8] = include_bytes!("favicon.ico");

pub async fn serve_index(doc_path: String) -> Result<impl warp::Reply, Infallible> {
    let content = INDEX_TEMPLATE.replace("__url_path__", &doc_path);
    Ok(warp::reply::html(content))
}


pub async fn favicon() -> Result<impl warp::Reply, Infallible> {
    Ok(FAVICON)
}
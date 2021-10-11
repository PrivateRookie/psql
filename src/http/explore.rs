use std::{convert::Infallible, ops::Deref, sync::Arc};

use futures::lock::Mutex;

use super::Plan;

pub async fn status(plan_db: Arc<Mutex<Plan>>) -> Result<impl warp::Reply, Infallible> {
    let plan = plan_db.lock().await;
    Ok(warp::reply::json(plan.deref()))
}

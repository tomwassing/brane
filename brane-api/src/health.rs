/* TIM */

/* HEALTH.rs
 *   by Lut99
 *
 * Created:
 *   12 Jan 2022, 13:29:01
 * Last edited:
 *   20 Jan 2022, 16:34:08
 * Auto updated?
 *   Yes
 *
 * Description:
 *   Contains code for the health part of the brane API.
**/

use warp::reply::Response;
use warp::http::HeaderValue;
use warp::hyper::Body;
use warp::{Reply, Rejection};


///
///
///
pub async fn health() -> Result<impl Reply, Rejection> {
    let mut response = Response::new(Body::from("OK!\n"));

    response.headers_mut().insert(
        "Content-Length",
        HeaderValue::from(4),
    );

    Ok(response)
}

/*******/

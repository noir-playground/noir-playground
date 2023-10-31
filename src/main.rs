#![deny(unreachable_pub, unused_crate_dependencies)]
#![deny(clippy::use_self)]

mod docker;
mod errors;
mod github;
mod routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = "0.0.0.0:8080".parse()?;
    axum::Server::bind(&addr).serve(routes::init().into_make_service()).await?;
    Ok(())
}

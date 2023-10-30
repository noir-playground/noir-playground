use anyhow::anyhow;
use axum::extract::Query;
use axum::routing::{get, get_service, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::services::ServeDir;

use crate::docker::Container;
use crate::errors::AppError;

type Result = anyhow::Result<Json<Value>, AppError>;

pub(crate) fn init() -> Router {
    let root_files = ServeDir::new("static").precompressed_gzip();
    Router::new()
        .fallback_service(get_service(root_files))
        .route("/api/:channel/check", post(check))
        .route("/api/:channel/compile", post(compile))
        .route("/api/:channel/execute", post(execute))
        .route("/api/:channel/fmt", post(fmt))
        .route("/api/:channel/version", get(version))
}

async fn version(container: Container) -> Result {
    let (stdout, stderr) = container.nargo(&["--version"]).await?;

    if !stderr.is_empty() {
        return Err(anyhow!("unknown error").into());
    }

    Ok(Json(json!({"version": stdout })))
}

async fn check(
    container: Container,
    Query(options): Query<Options>,
    Json(files): Json<Files>,
) -> Result {
    eval("check", container, files, options).await
}

async fn compile(
    container: Container,
    Query(options): Query<Options>,
    Json(files): Json<Files>,
) -> Result {
    eval("compile", container, files, options).await
}

async fn execute(
    container: Container,
    Query(options): Query<Options>,
    Json(files): Json<Files>,
) -> Result {
    eval("execute", container, files, options).await
}

async fn fmt(container: Container, Json(files): Json<Files>) -> Result {
    container.write_source(files.code)?;
    container.write_prover(files.input)?;

    let _ = container.nargo(&["fmt"]).await?;
    let code = container.read_source()?;

    Ok(json!({"code": code}).into())
}

async fn eval(
    command: &str,
    container: Container,
    files: Files,
    options: Options,
) -> std::result::Result<Json<Value>, AppError> {
    container.write_source(files.code)?;
    container.write_prover(files.input)?;

    let args: Vec<_> = Some(command).into_iter().chain(options.args()).collect();

    let (stdout, stderr) = container.nargo(&args).await?;
    nargo_response(container, stdout, stderr).await
}

async fn nargo_response(container: Container, stdout: String, stderr: String) -> Result {
    let (compiler, _) = container.nargo(&["--version"]).await?;

    Ok(json!({"compiler": compiler, "stdout": stdout, "stderr": stderr }).into())
}

#[derive(Deserialize)]
struct Files {
    code: String,
    #[serde(default)]
    input: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Options {
    #[serde(default)]
    show_ssa: bool,
    #[serde(default)]
    deny_warnings: bool,
    #[serde(default)]
    silence_warnings: bool,
    #[serde(default)]
    print_acir: bool,
}

impl Options {
    fn args(self) -> Vec<&'static str> {
        let mut args = Vec::new();

        if self.show_ssa {
            args.push("--show-ssa");
        }

        if self.deny_warnings {
            args.push("--deny-warnings");
        }

        if self.silence_warnings {
            args.push("--silence-warnings");
        }

        if self.print_acir {
            args.push("--print-acir");
        }

        args
    }
}

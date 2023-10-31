use std::mem::take;

use anyhow::anyhow;
use axum::extract::{Path, Query};
use axum::routing::{get, get_service, post, MethodRouter};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::ServeDir;

use crate::docker::Container;
use crate::errors::AppError;
use crate::github::Client;

type Result<T = Value> = anyhow::Result<Json<T>, AppError>;

const MAIN: &str = "main.nr";
const PROVER: &str = "Prover.nr";

pub(crate) fn init() -> Router {
    let github_token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let root_files = ServeDir::new("static").precompressed_gzip();

    Router::new()
        .fallback_service(get_service(root_files))
        .route("/api/gist", post(gist_create))
        .route("/api/gist/:id", get(gist_view))
        .route("/api/nargo/:channel/check", mk_service("check"))
        .route("/api/nargo/:channel/compile", mk_service("compile"))
        .route("/api/nargo/:channel/execute", mk_service("execute"))
        .route("/api/nargo/:channel/fmt", post(fmt))
        .route("/api/nargo/:channel/version", get(version))
        .layer(Extension(Client::new(github_token)))
}

fn mk_service(command: &'static str) -> MethodRouter {
    post(|container: Container, Query(options): Query<Options>, Json(files): Json<Files>| async {
        eval(command, container, files, options).await
    })
}

async fn gist_create(Extension(client): Extension<Client>, Json(files): Json<Files>) -> Result {
    let github = client.load()?;
    let gist = github
        .gists()
        .create()
        .file(MAIN, files.code)
        .file(PROVER, files.input)
        .public(false)
        .send()
        .await?;

    Ok(json!({ "id": gist.id }).into())
}

async fn gist_view(Path(id): Path<String>, Extension(client): Extension<Client>) -> Result<Files> {
    let github = client.load()?;

    let mut files = github.gists().get(id).await?.files;

    let code = files.get_mut(MAIN).and_then(|file| take(&mut file.content)).unwrap_or_default();
    let input = files.get_mut(PROVER).and_then(|file| take(&mut file.content)).unwrap_or_default();

    Ok(Files { code, input }.into())
}

async fn version(container: Container) -> Result {
    let (stdout, stderr) = container.nargo(&["--version"]).await?;

    if !stderr.is_empty() {
        return Err(anyhow!("unknown error").into());
    }

    Ok(Json(json!({"version": stdout })))
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

#[derive(Deserialize, Serialize)]
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

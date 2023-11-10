use std::collections::HashMap;
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
const PROVER: &str = "Prover.toml";

pub(crate) fn init() -> Router {
    let github_token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let root_files = ServeDir::new("dist").precompressed_gzip();

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
    post(|container: Container, Query(options): Query<Options>, Json(files): Json<Project>| async {
        eval(command, container, files, options).await
    })
}

async fn gist_create(Extension(client): Extension<Client>, Json(project): Json<Project>) -> Result {
    let mut files = project.files;
    let github = client.load()?;

    let main = files.get_mut(MAIN).map(take).unwrap_or_default().content;
    let prover = files.get_mut(PROVER).map(take).unwrap_or_default().content;

    let gist =
        github.gists().create().file(MAIN, main).file(PROVER, prover).public(false).send().await?;

    Ok(json!({ "id": gist.id }).into())
}

async fn gist_view(
    Path(id): Path<String>,
    Extension(client): Extension<Client>,
) -> Result<Project> {
    let github = client.load()?;

    let files = github.gists().get(id).await?.files;
    let files = files
        .into_iter()
        .map(|(key, value)| (key, File { content: value.content.unwrap_or_default() }))
        .collect();

    Ok(Project { files }.into())
}

async fn version(container: Container) -> Result {
    let (stdout, stderr) = container.nargo(&["--version"]).await?;

    if !stderr.is_empty() {
        return Err(anyhow!("unknown error").into());
    }

    Ok(json!({"version": stdout }).into())
}

async fn fmt(container: Container, Json(mut project): Json<Project>) -> Result {
    container.write_source(project.take_source().content)?;
    container.write_prover(project.take_prover().content)?;

    let _ = container.nargo(&["fmt"]).await?;
    let code = container.read_source()?;

    Ok(json!({"code": code}).into())
}

async fn eval(
    command: &str,
    container: Container,
    mut files: Project,
    options: Options,
) -> std::result::Result<Json<Value>, AppError> {
    container.write_source(files.take_source().content)?;
    container.write_prover(files.take_prover().content)?;

    let args: Vec<_> = Some(command).into_iter().chain(options.args()).collect();

    let (stdout, stderr) = container.nargo(&args).await?;

    let stdout = ansi_to_html::convert_escaped(&stdout).unwrap_or(stdout);
    let stderr = ansi_to_html::convert_escaped(&stderr).unwrap_or(stderr);

    nargo_response(stdout, stderr).await
}

async fn nargo_response(stdout: String, stderr: String) -> Result {
    Ok(json!({"stdout": stdout, "stderr": stderr }).into())
}

#[derive(Deserialize, Serialize)]
struct Project {
    files: HashMap<String, File>,
}

#[derive(Default, Deserialize, Serialize)]
struct File {
    content: String,
}

impl Project {
    fn take_source(&mut self) -> File {
        self.files.get_mut(MAIN).map(take).unwrap_or_default()
    }

    fn take_prover(&mut self) -> File {
        self.files.get_mut(PROVER).map(take).unwrap_or_default()
    }
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

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::RequestPartsExt;
use serde::Deserialize;
use tempdir::TempDir;
use tokio::process::Command;

use crate::errors::AppError;

pub(crate) struct Container {
    channel: Channel,
    #[allow(dead_code)]
    temp: TempDir,
    source_file: PathBuf,
    prover_file: PathBuf,
}

impl Container {
    pub(crate) fn new(channel: Channel) -> Result<Self> {
        TempDir::new("playground")
            .map(|temp| Self {
                source_file: temp.path().join("main.nr"),
                prover_file: temp.path().join("Prover.toml"),
                channel,
                temp,
            })
            .map_err(|_| anyhow!("failed to create temporary directory"))
    }

    pub(crate) fn read_source(&self) -> Result<String> {
        std::fs::read_to_string(&self.source_file)
            .map_err(|_| anyhow!("failed to read source code"))
    }

    pub(crate) fn write_source(&self, code: String) -> Result<()> {
        std::fs::write(&self.source_file, code).map_err(|_| anyhow!("failed to write source code"))
    }

    pub(crate) fn write_prover(&self, code: String) -> Result<()> {
        std::fs::write(&self.prover_file, code).map_err(|_| anyhow!("failed to write Prover.toml"))
    }

    pub(crate) async fn nargo(&self, args: &[&str]) -> Result<(String, String)> {
        let mut mount_source_file = self.source_file.as_os_str().to_os_string();
        mount_source_file.push(":");
        mount_source_file.push("/playground/src/main.nr");

        let mut mount_prover_file = self.prover_file.as_os_str().to_os_string();
        mount_prover_file.push(":");
        mount_prover_file.push("/playground/Prover.toml");

        let output = Command::new("docker")
            .arg("run")
            .args(["--cap-drop=ALL", "-i", "--rm"])
            .args(["--platform", "linux/amd64"])
            .args(["--net", "none"])
            .args(["--memory", "512m"])
            .args(["--memory-swap", "640m"])
            .args(["--pids-limit", "512"])
            .args(["--oom-score-adj", "1000"])
            .args(["-a", "stdin", "-a", "stdout", "-a", "stderr"])
            .arg("--volume")
            .arg(&mount_source_file)
            .arg("--volume")
            .arg(&mount_prover_file)
            .arg(self.channel.container_name())
            .arg("nargo")
            .args(args)
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|_| anyhow!("failed to start docker container"))?;

        Ok((
            String::from_utf8(output.stdout).unwrap_or_default(),
            String::from_utf8(output.stderr).unwrap_or_default(),
        ))
    }
}

#[async_trait::async_trait]
impl FromRequestParts<()> for Container {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _: &()) -> Result<Self, Self::Rejection> {
        let channel =
            parts.extract::<Path<Channel>>().await.map_err(IntoResponse::into_response)?;
        Self::new(channel.0).map_err(AppError).map_err(IntoResponse::into_response)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Channel {
    Master,
}

impl Channel {
    fn container_name(&self) -> &'static str {
        match self {
            Self::Master => "noir-master",
        }
    }
}

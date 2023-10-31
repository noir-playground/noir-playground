use std::sync::Arc;

use anyhow::Result;
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub(crate) struct Client(pub(crate) Arc<SecretString>);

impl Client {
    pub(crate) fn new(value: String) -> Self {
        Self(Arc::new(value.into()))
    }
    pub(crate) fn load(&self) -> Result<octocrab::Octocrab> {
        let personal_token = self.0.as_ref().expose_secret().clone();
        octocrab::OctocrabBuilder::new().personal_token(personal_token).build().map_err(Into::into)
    }
}

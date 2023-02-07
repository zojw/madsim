use aws_sdk_sts::model::Credentials as StsCredentials;
use aws_types::credentials::{self, CredentialsError};
use aws_types::Credentials as AwsCredentials;

use std::convert::TryFrom;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn into_credentials(
    sts_credentials: Option<StsCredentials>,
    provider_name: &'static str,
) -> credentials::Result {
    let sts_credentials = sts_credentials
        .ok_or_else(|| CredentialsError::unhandled("STS credentials must be defined"))?;
    let expiration = SystemTime::try_from(
        sts_credentials
            .expiration
            .ok_or_else(|| CredentialsError::unhandled("missing expiration"))?,
    )
    .map_err(|_| {
        CredentialsError::unhandled(
            "credential expiration time cannot be represented by a SystemTime",
        )
    })?;
    Ok(AwsCredentials::new(
        sts_credentials
            .access_key_id
            .ok_or_else(|| CredentialsError::unhandled("access key id missing from result"))?,
        sts_credentials
            .secret_access_key
            .ok_or_else(|| CredentialsError::unhandled("secret access token missing"))?,
        sts_credentials.session_token,
        Some(expiration),
        provider_name,
    ))
}

pub(crate) fn default_session_name(base: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("post epoch");
    format!("{}-{}", base, now.as_millis())
}
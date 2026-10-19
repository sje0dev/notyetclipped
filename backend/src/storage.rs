use std::{path::Path, time::Duration};

use anyhow::{Result, ensure};
use aws_sdk_s3::{
    Client,
    config::{Credentials, Region},
    presigning::PresigningConfig,
    primitives::ByteStream,
};

use crate::config::Config;

#[derive(Clone)]
pub struct Storage {
    pub client: Client,
    signer: Client,
    pub bucket: String,
}

impl Storage {
    pub fn new(config: &Config) -> Self {
        let build = |endpoint: &Option<String>| {
            let mut builder = aws_sdk_s3::config::Builder::new()
                .behavior_version_latest()
                .region(Region::new(config.s3_region.clone()))
                .credentials_provider(Credentials::new(
                    &config.s3_access_key,
                    &config.s3_secret_key,
                    None,
                    None,
                    "environment",
                ))
                .force_path_style(true)
                .timeout_config(
                    aws_sdk_s3::config::timeout::TimeoutConfig::builder()
                        .connect_timeout(Duration::from_secs(4))
                        .operation_timeout(Duration::from_secs(180))
                        .build(),
                );
            if let Some(endpoint) = endpoint {
                builder = builder.endpoint_url(endpoint);
            }
            Client::from_conf(builder.build())
        };
        Self {
            client: build(&config.s3_endpoint),
            signer: build(&config.s3_public_endpoint),
            bucket: config.s3_bucket.clone(),
        }
    }

    pub async fn check(&self) -> Result<()> {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.client.head_bucket().bucket(&self.bucket).send(),
        )
        .await??;
        Ok(())
    }

    pub async fn upload(&self, path: &Path, key: &str, content_type: &str) -> Result<()> {
        let size = tokio::fs::metadata(path).await?.len();
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from_path(path).await?)
            .send()
            .await?;
        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        ensure!(
            head.content_length() == Some(size as i64),
            "S3-Upload konnte nicht vollständig bestätigt werden."
        );
        Ok(())
    }

    pub async fn url(&self, key: &str, filename: Option<&str>) -> Result<String> {
        let mut request = self.signer.get_object().bucket(&self.bucket).key(key);
        if let Some(filename) = filename {
            request = request
                .response_content_disposition(format!("attachment; filename=\"{filename}\""));
        }
        Ok(request
            .presigned(PresigningConfig::expires_in(Duration::from_secs(3600))?)
            .await?
            .uri()
            .to_owned())
    }

    pub async fn delete(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await?;
        }
        Ok(())
    }

    pub async fn initialize_local(&self, config: &Config) -> Result<()> {
        if config.s3_endpoint.as_deref() != Some("http://s3:9000") {
            return Ok(());
        }
        for attempt in 0..30 {
            if self.check().await.is_ok() {
                return Ok(());
            }
            let mut request = self.client.create_bucket().bucket(&self.bucket);
            if config.s3_region != "us-east-1" {
                request = request.create_bucket_configuration(
                    aws_sdk_s3::types::CreateBucketConfiguration::builder()
                        .location_constraint(aws_sdk_s3::types::BucketLocationConstraint::from(
                            config.s3_region.as_str(),
                        ))
                        .build(),
                );
            }
            match request.send().await {
                Ok(_) => return Ok(()),
                Err(error) if attempt == 29 => return Err(error.into()),
                Err(_) => tokio::time::sleep(Duration::from_secs(2)).await,
            }
        }
        Ok(())
    }
}

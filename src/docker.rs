use std::{
    error::Error,
    fs::File,
    io::{self, Cursor},
    pin::Pin,
};
use bollard::{
    Docker,
    auth::DockerCredentials,
    image::{BuildImageOptions, TagImageOptions},
};
use futures_util::{Stream, StreamExt, TryStreamExt};
use http_body::Frame;
use http_body_util::{Either, StreamBody};
use std::path::Path;
use tar::{Builder, Header};
use tokio_util::codec::BytesCodec;
use tokio_util::{bytes, codec::FramedRead};
use walkdir::WalkDir;

use axum::body::Bytes;
use bollard::image::PushImageOptions;
use bytes::Bytes as BytesRaw;

// TODO:
// Manually parse or enter the .dockerignore and also
// make sure the root is correct (should it be ./Dockefile or within gameserver/Dockerfile?)


const ENABLE_TAG_AND_PUSH: bool = true;

#[derive(Debug)]
struct DockerBuildError(String);

impl std::fmt::Display for DockerBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Docker build error: {}", self.0)
    }
}

impl std::error::Error for DockerBuildError {}

pub async fn build_docker_image() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let docker =
        Docker::connect_with_unix("/var/run/docker.sock", 120, bollard::API_DEFAULT_VERSION)?;
    let context_path = Path::new("gameserver");
    let mut archive = Vec::new();
    {
        let mut tar = Builder::new(&mut archive);

        for entry in WalkDir::new(context_path)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !path.is_file() && !path.is_dir() {
                continue;
            }
            if path.is_symlink() {
                continue;
            }

            let relative_path = path.strip_prefix(context_path)?;
            if let Some(first_component) = relative_path.components().next() {
                if first_component.as_os_str() == "target" {
                    continue;
                }
            }
            if relative_path.starts_with("gameserver/target") {
                continue;
            }
            if relative_path
                .components()
                .next()
                .map(|c| c.as_os_str() == "server")
                .unwrap_or(false)
            {
                continue;
            }

            if let Some(relative_str) = relative_path.to_str() {
                if relative_str.starts_with('.') || relative_str.contains("/.") {
                    continue;
                }
            }

            if path.is_dir() {
                let mut header = Header::new_gnu();
                header.set_size(0);
                header.set_entry_type(tar::EntryType::Directory);
                header.set_path(relative_path)?;
                header.set_cksum();
                tar.append(&header, &mut io::empty())?;
            } else {
                let mut file = File::open(path)?;
                let mut header = Header::new_gnu();
                header.set_path(relative_path)?;
                header.set_size(file.metadata()?.len());
                header.set_mode(0o644);
                header.set_cksum();
                tar.append(&header, &mut file)?;
            }
        }

        tar.finish()?;
    }

    let cursor = Cursor::new(archive);
    let framed: FramedRead<Cursor<Vec<u8>>, BytesCodec> =
        FramedRead::new(cursor, BytesCodec::new());

    let stream_converted = framed
        .map_ok(|bytes_mut| {
            let b: BytesRaw = bytes_mut.freeze();
            Bytes::from(b)
        })
        .map_ok(|b: Bytes| {
            Frame::data(b)
        })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));

    let boxed_stream: Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, io::Error>> + Send>> =
        Box::pin(stream_converted);

    let stream_body = StreamBody::new(boxed_stream);

    let repo = std::env::var("DOCKER_REPO");
    println!(
        "Repo is: {}",
        repo.clone().unwrap_or("Nothing here".to_string())
    );

    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: &format!(
            "{}/gameserver:latest",
            repo.clone().unwrap_or("".to_string())
        ),
        rm: true,
        forcerm: true,
        ..Default::default()
    };

    let mut build_stream = docker.build_image(options, None, Some(Either::Right(stream_body)));

    while let Some(update) = build_stream.next().await {
        match update {
            Ok(update) => {
                if let Some(msg) = update.stream {
                    print!("{}", msg);
                }
                if let Some(err) = update.error {
                    return Err(DockerBuildError(err).into());
                }
            }
            Err(e) => {
                return Err(DockerBuildError(format!("Docker API error: {}", e)).into());
            }
        }
    }

    if ENABLE_TAG_AND_PUSH && repo.clone().is_ok() {
        println!("Tagging and pushing repo");
        docker
            .tag_image(
                &format!(
                    "{}/gameserver:latest",
                    repo.clone().unwrap_or("".to_string())
                ),
                Some(TagImageOptions {
                    repo: format!(
                        "{}/gameserver",
                        repo.clone().unwrap_or("localhost:5000/".to_string())
                    ),
                    tag: "latest".to_string(),
                }),
            )
            .await?;

        docker
            .push_image(
                &format!(
                    "{}/gameserver",
                    repo.clone().unwrap_or("localhost:5000/".to_string())
                ),
                None::<PushImageOptions<String>>,
                None::<DockerCredentials>,
            )
            .try_collect::<Vec<_>>()
            .await?;
    }

    println!("\nDocker image built successfully!");
    Ok(())
}

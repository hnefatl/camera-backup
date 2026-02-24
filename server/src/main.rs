use std::collections::HashMap;

use anyhow::bail;
use lib::proto::{
    ExistsRequest, ExistsResponse, SendRequest, SendResponse,
    camera_backup_server::{CameraBackup, CameraBackupServer},
};
use log::{debug, error, info};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tonic::{Request, Response, Status};

mod args;
use args::ARGS;

// Helper for converting `anyhow::Result<T>` into a `Result<T, Status>`.
trait IntoStatus<T> {
    fn into_status<F>(self, f: F) -> Result<T, Status>
    where
        F: FnOnce(String) -> Status;
}
impl<T> IntoStatus<T> for anyhow::Result<T> {
    fn into_status<F>(self, f: F) -> Result<T, Status>
    where
        F: FnOnce(String) -> Status,
    {
        self.map_err(|e| f(format!("{:#}", e)))
    }
}

#[derive(Debug, Default)]
pub struct Server {
    // Filename like `IMG_6812.jpg` to the date components of the path.
    filenames: Mutex<HashMap<String, lib::Date>>,
}
impl Server {
    fn init_from_directory_tree(directory: &str) -> anyhow::Result<Self> {
        info!("Walking existing directory tree");
        let walker = walkdir::WalkDir::new(directory);
        let mut filenames = HashMap::new();
        for entry in walker.into_iter() {
            let entry = entry?;
            if !lib::is_image_file(&entry) {
                continue;
            }
            let date = lib::Date::from_path(entry.path())?;
            let Some(decoded_filename) = entry.file_name().to_str() else {
                bail!("Unable to decode filename: {:?}", entry.file_name());
            };
            filenames.insert(decoded_filename.to_owned(), date);
        }
        info!(
            "Finished processing existing directory tree, found {} files",
            filenames.len()
        );
        Ok(Self {
            filenames: Mutex::new(filenames),
        })
    }
}
#[tonic::async_trait]
impl CameraBackup for Server {
    async fn exists(&self, request: Request<ExistsRequest>) -> Result<Response<ExistsResponse>, Status> {
        info!(
            "got Exists call from {:?} for {}",
            request.remote_addr(),
            request.get_ref().filename
        );
        let filenames = self.filenames.lock().await;
        Ok(Response::new(ExistsResponse {
            exists: filenames.contains_key(&request.get_ref().filename),
        }))
    }
    async fn send(
        &self,
        mut request: Request<tonic::Streaming<SendRequest>>,
    ) -> Result<Response<SendResponse>, Status> {
        let mut message = request
            .get_mut()
            .message()
            .await?
            .ok_or(Status::invalid_argument("no initial message"))?;
        let filename = message
            .filename
            .ok_or(Status::invalid_argument("initial message had `filename` unset"))?;
        debug!("got Send call from {:?} for {}", request.remote_addr(), filename,);
        let created = message
            .created
            .ok_or(Status::invalid_argument("initial message had `created` unset"))?;

        let date: lib::Date = created.try_into().into_status(Status::invalid_argument)?;
        let dest_path = date.to_output_file(&ARGS.directory, &filename);
        if let Some(p) = dest_path.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        let mut f = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest_path.clone())
            .await?;

        loop {
            f.write(&message.contents).await?;
            let Some(m) = request.get_mut().message().await? else {
                break;
            };
            message = m;
        }
        if let Err(e) = f.sync_all().await {
            error!("Failed syncing {} to disk: {}", dest_path.display(), e);
        }
        let mut filenames = self.filenames.lock().await;
        filenames.insert(filename.clone(), date);
        info!("Wrote {}, now tracking {} files.", dest_path.display(), filenames.len());
        Ok(Response::new(SendResponse {}))
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("INFO"));
    debug!("Args: {:?}", *ARGS);

    let server = Server::init_from_directory_tree(&ARGS.directory)?;
    let address = ARGS.address.parse()?;

    info!("Listening on {}", address);
    tonic::transport::Server::builder()
        .add_service(CameraBackupServer::new(server))
        .serve(address)
        .await?;

    Ok(())
}

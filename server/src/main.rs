use tonic::{Request, Response, Status};

use lib::proto::{
    ExistsRequest, ExistsResponse, SendRequest, SendResponse,
    camera_backup_server::{CameraBackup, CameraBackupServer},
};

#[derive(Debug, Default)]
pub struct Server {}

#[tonic::async_trait]
impl CameraBackup for Server {
    async fn exists(&self, _request: Request<ExistsRequest>) -> Result<Response<ExistsResponse>, Status> {
        unimplemented!()
    }
    async fn send(&self, _request: Request<SendRequest>) -> Result<Response<SendResponse>, Status> {
        unimplemented!()
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let greeter = Server::default();

    tonic::transport::Server::builder()
        .add_service(CameraBackupServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}

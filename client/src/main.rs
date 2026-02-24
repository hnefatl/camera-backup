use anyhow::bail;
use lib::proto::camera_backup_client::CameraBackupClient;
use lib::proto::{ExistsRequest, SendRequest};
use log::{debug, error, info};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio_stream::Stream;
use tonic::transport::Endpoint;

mod args;
use args::ARGS;

use crate::notifier::Notifier;

mod notifier;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("INFO"));
    debug!("Args: {:?}", *ARGS);

    let address = Endpoint::try_from(ARGS.server_address.clone())?;
    let client = CameraBackupClient::connect(address).await?;

    let counters = Arc::new(Mutex::new(Counters {
        found: 0,
        exist: 0,
        sent: 0,
    }));
    let task_config = TaskConfig {
        client,
        inflight_sends: Arc::new(Semaphore::new(ARGS.max_inflight_sends)),
        counters: counters.clone(),
    };

    let start_time = Instant::now();
    let mut join_handles = JoinSet::new();

    let shutdown_notifier = Arc::new(AtomicBool::new(false));
    let notifier_thread = if ARGS.send_notifications {
        let notifier = shutdown_notifier.clone();
        let counters = counters.clone();
        Some(std::thread::spawn(|| notifier_task(notifier, counters)))
    } else {
        None
    };

    let mut num_failed_tasks = 0;
    for e in walkdir::WalkDir::new(&ARGS.source_root) {
        let e = e?;
        if !lib::is_image_file(&e) {
            debug!("Skipping non-image: {}", e.path().display());
            continue;
        }
        debug!("[{}] found {}", counters.lock().await, e.path().display());

        join_handles.spawn(handle_file_task(task_config.clone(), e.path().to_path_buf()));
        counters.lock().await.found += 1;
    }
    info!("Finished scanning files in {:#?}", Instant::now() - start_time);
    let results = join_handles.join_all().await;
    info!("All tasks completed after in {:#?}", Instant::now() - start_time);
    num_failed_tasks += results.iter().flatten().count();
    if num_failed_tasks > 0 {
        bail!("{} tasks failed", num_failed_tasks);
    }
    if let Some(n) = notifier_thread {
        shutdown_notifier.store(true, Ordering::Relaxed);
        match n.join() {
            Err(e) => error!("failed to join notifier thread: {:?}", e),
            Ok(j) => j?,
        }
    }
    debug!("Shutting down notifier");
    Ok(())
}

#[derive(Debug, Clone)]
struct Counters {
    found: u32,
    exist: u32,
    sent: u32,
}
impl std::fmt::Display for Counters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "found: {}, exist: {}, sent: {}",
            self.found, self.exist, self.sent
        ))
    }
}
#[derive(Clone)]
struct TaskConfig {
    client: CameraBackupClient<tonic::transport::Channel>,
    inflight_sends: Arc<Semaphore>,
    counters: Arc<Mutex<Counters>>,
}

async fn handle_file_task(config: TaskConfig, path: PathBuf) -> Option<anyhow::Error> {
    if let Err(e) = handle_file(config, &path).await {
        error!("[{}] {}", path.display(), e);
        return Some(e);
    }
    None
}
async fn handle_file(mut config: TaskConfig, path: &Path) -> anyhow::Result<()> {
    let filename = path.file_name().unwrap().to_str().unwrap().to_string();
    let exists_request = ExistsRequest {
        filename: filename.clone(),
    };

    debug!("Sending Exists request for {}", path.display());
    let exists_response = config.client.exists(exists_request).await?;
    {
        let mut counters = config.counters.lock().await;
        if exists_response.get_ref().exists {
            counters.exist += 1;
            debug!("[{}] {} already exists", counters, path.display());
            return Ok(());
        }
        debug!("[{}] {} doesn't exist", counters, path.display());
    }

    let _permit = config.inflight_sends.acquire().await?;
    if !ARGS.dry_run {
        debug!(
            "[{}] Sending Send request for {}",
            config.counters.lock().await,
            path.display()
        );

        let read_start = Instant::now();
        let contents = tokio::fs::read(path.to_path_buf()).await?;
        let read_time = Instant::now() - read_start;
        let stream = stream_file(filename, contents)?;
        let send_start = Instant::now();
        config.client.send(stream).await?;
        let send_time = Instant::now() - send_start;

        let mut counters = config.counters.lock().await;
        counters.sent += 1;
        info!(
            "[{}] Sent {} (file read: {:.02}s, send: {:.02}s)",
            counters,
            path.display(),
            read_time.as_secs_f32(),
            send_time.as_secs_f32()
        );
    } else {
        info!(
            "[dry run] [{}] Sent {}",
            config.counters.lock().await,
            path.display()
        );
    }
    Ok(())
}

fn stream_file(filename: String, contents: Vec<u8>) -> anyhow::Result<impl Stream<Item = SendRequest>> {
    let mut chunks = contents.chunks(ARGS.chunk_size).map(|c| c.to_vec());
    let Some(init_chunk) = chunks.next() else {
        // Could just send empty contents, but this is probably something to be alerted on.
        bail!("Empty file");
    };
    let mut requests = vec![SendRequest {
        filename: Some(filename),
        created: Some(lib::Date::from_file_exif(contents.clone())?.into()),
        contents: init_chunk,
    }];
    for chunk in chunks {
        requests.push(SendRequest {
            filename: None,
            created: None,
            contents: chunk,
        });
    }
    Ok(tokio_stream::iter(requests))
}

fn notifier_task(shutdown: Arc<AtomicBool>, counters: Arc<Mutex<Counters>>) -> anyhow::Result<()> {
    let n = Notifier::new(ARGS.send_notifications)?;
    while !shutdown.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
        let c = counters.blocking_lock();
        n.update(c.sent, c.found - c.exist)?;
    }
    n.signoff()?;
    Ok(())
}

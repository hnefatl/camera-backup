use log::{debug, error, info};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

mod args;
use args::ARGS;

mod counted_channel;
mod notifier;

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("INFO"));
    debug!("Args: {:?}", *ARGS);

    let (sender, receiver, cancel) = counted_channel::channel(ARGS.queue_capacity);

    ctrlc::set_handler(cancel)?;

    let finder_handle = thread::spawn(|| find_new_files(sender));
    let copier_handle = thread::spawn(|| copy_files(receiver));

    let mut result: anyhow::Result<()> = Ok(());
    if let Err(e) = finder_handle.join().unwrap() {
        error!("file finder: {}", e);
        result = Err(e);
    }
    if let Err(e) = copier_handle.join().unwrap() {
        error!("file copier: {}", e);
        result = Err(e);
    }
    result
}

#[derive(Debug)]
struct CopyOp {
    source_path: PathBuf,
    destination_path: PathBuf,
}
impl CopyOp {
    fn needs_copying(path: &Path) -> anyhow::Result<Option<Self>> {
        let source_datetime = datetime_from_file(path)?;
        // E.g. "/tmp/foo/2026/01/IMG_foo.jpg"
        let destination_path = PathBuf::new()
            .join(&ARGS.destination_root)
            .join(format!("{}", source_datetime.year))
            .join(format!("{:02}", source_datetime.month))
            .join(path.file_name().unwrap());

        if destination_path.try_exists()? {
            return Ok(None);
        }
        Ok(Some(Self {
            source_path: path.to_path_buf(),
            destination_path,
        }))
    }
}

fn find_new_files(sender: counted_channel::Sender<CopyOp>) -> anyhow::Result<()> {
    let start_time = Instant::now();
    for e in walkdir::WalkDir::new(&ARGS.source_root) {
        let e = e?;
        if !is_image_file(&e) {
            debug!("Skipping non-image: {}", e.path().display());
            continue;
        }

        if let Some(o) = CopyOp::needs_copying(e.path())? {
            debug!("{} does need copying", e.path().display());
            sender.send(o)?;
        } else {
            debug!("{} doesn't need copying", e.path().display());
        }
    }
    sender.finish();
    info!("Finished scanning files in {:#?}", Instant::now() - start_time);
    Ok(())
}

fn copy_files(receiver: counted_channel::Receiver<CopyOp>) -> anyhow::Result<()> {
    let mut copied_files = 0;
    let notifier = notifier::Notifier::new(ARGS.send_notifications)?;

    let mut start_time = None;
    while let Some(f) = receiver.recv()? {
        start_time.get_or_insert_with(Instant::now);

        copied_files += 1;
        let total_files = copied_files + receiver.len();
        info!(
            "{}Copying {}/{}: {} to {}",
            if ARGS.dry_run { "[dry run] " } else { "" },
            copied_files,
            total_files,
            f.source_path.display(),
            f.destination_path.display()
        );
        notifier.update(copied_files, total_files)?;
        if !ARGS.dry_run {
            if let Some(parent) = f.destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(f.source_path, f.destination_path)?;
        }
    }
    if let Some(start_time) = start_time {
        info!("Finished copying files in {:#?}", Instant::now() - start_time);
    }
    notifier.signoff()?;
    Ok(())
}

fn is_image_file(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    let Some(extension) = entry.path().extension() else {
        return false;
    };
    let lower = extension.to_ascii_lowercase();
    lower == "cr2" || lower == "jpg"
}

fn datetime_from_file(path: &Path) -> anyhow::Result<exif::DateTime> {
    let f = std::fs::File::open(path)?;
    let mut br = std::io::BufReader::new(f);
    let er = exif::Reader::new();
    let e = er.read_from_container(&mut br)?;
    let Some(field) = e.get_field(exif::Tag::DateTime, exif::In::PRIMARY) else {
        anyhow::bail!("[{}] No datetime field", path.display());
    };
    let exif::Value::Ascii(ref d) = field.value else {
        anyhow::bail!(
            "[{}] Non-ASCII value in datetime field: {:?}",
            path.display(),
            field.value
        );
    };
    let Some(d) = d.first() else {
        anyhow::bail!("[{}] Missing data in datetime field", path.display());
    };
    Ok(exif::DateTime::from_ascii(d)?)
}

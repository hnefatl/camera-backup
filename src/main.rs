use log::{debug, error, info};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

mod args;
use args::ARGS;

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("INFO"));
    debug!("Args: {:?}", *ARGS);

    let (sender, receiver) = mpsc::sync_channel(ARGS.queue_capacity);

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

fn find_new_files(sender: mpsc::SyncSender<CopyOp>) -> anyhow::Result<()> {
    let mut total_image_files = 0;
    let mut new_image_files = 0;
    for e in walkdir::WalkDir::new(&ARGS.source_root) {
        let e = e?;
        if !is_image_file(&e) {
            debug!("Skipping non-image");
            continue;
        }
        total_image_files += 1;

        if let Some(o) = CopyOp::needs_copying(e.path())? {
            debug!("{} does need copying", e.path().display());
            sender.send(o)?;
            new_image_files += 1;
        } else {
            debug!("{} doesn't need copying", e.path().display());
        }
    }

    info!(
        "Found {} total images, {} new images",
        total_image_files, new_image_files
    );
    Ok(())
}

fn copy_files(receiver: mpsc::Receiver<CopyOp>) -> anyhow::Result<()> {
    while let Ok(f) = receiver.recv() {
        info!(
            "{}Copying {} to {}",
            if ARGS.dry_run { "[dry run] " } else { "" },
            f.source_path.display(),
            f.destination_path.display()
        );
        if !ARGS.dry_run {
            if let Some(parent) = f.destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(f.source_path, f.destination_path)?;
        }
    }
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
    return lower == "cr2" || lower == "jpg";
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
    let Some(d) = d.get(0) else {
        anyhow::bail!("[{}] Missing data in datetime field", path.display());
    };
    Ok(exif::DateTime::from_ascii(d)?)
}

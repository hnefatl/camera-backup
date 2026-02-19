use std::{
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use chrono::{DateTime, Datelike};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/camera_backup.rs"));
}

#[derive(Debug)]
pub struct Date {
    year: u16,
    // Starting from 1=January
    month: u8,
}
impl Date {
    pub fn new(year: i64, month: u32) -> anyhow::Result<Self> {
        // Weird parameter types just ensures that we can `x as i64` in places
        // without potentially failing a cast.
        if !(month >= 1 && month <= 12) {
            bail!("invalid month: {}", month);
        }
        Ok(Date {
            year: year.try_into()?,
            month: month as u8,
        })
    }

    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let Some(month) = path.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) else {
            bail!("expected parent for {}", path.display(),);
        };
        let Some(year) = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
        else {
            bail!("expected grandparent for {}", path.display(),);
        };
        Self::new(
            year.parse().context(format!("failed to parse year: {}", year))?,
            month.parse().context(format!("failed to parse month: {}", month))?,
        )
    }
    pub fn from_timestamp(timestamp: prost_types::Timestamp) -> anyhow::Result<Self> {
        let Some(created) = DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32) else {
            bail!("can't parse timestamp: {}", timestamp);
        };
        Self::new(created.year() as i64, created.month())
    }
    pub fn from_file_exif(contents: Vec<u8>) -> anyhow::Result<Self> {
        let mut reader = BufReader::new(Cursor::new(contents));
        let er = exif::Reader::new();
        let e = er.read_from_container(&mut reader)?;
        let Some(field) = e.get_field(exif::Tag::DateTime, exif::In::PRIMARY) else {
            anyhow::bail!("No datetime field");
        };
        let exif::Value::Ascii(ref d) = field.value else {
            anyhow::bail!("Non-ASCII value in datetime field: {:?}", field.value);
        };
        let Some(d) = d.first() else {
            anyhow::bail!("Missing data in datetime field");
        };
        let exif = exif::DateTime::from_ascii(d)?;
        Self::new(exif.year as i64, exif.month as u32)
    }
    pub fn to_output_file(&self, root: &String, filename: &str) -> PathBuf {
        PathBuf::new()
            .join(root)
            .join(format!("{}", self.year))
            .join(format!("{:02}", self.month))
            .join(filename)
    }
}
impl TryFrom<proto::Date> for Date {
    type Error = anyhow::Error;
    fn try_from(value: proto::Date) -> Result<Self, Self::Error> {
        Self::new(value.year as i64, value.month)
    }
}
impl From<Date> for proto::Date {
    fn from(value: Date) -> Self {
        proto::Date {
            year: value.year as u32,
            month: value.month as u32,
        }
    }
}

pub fn is_image_file(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    let Some(extension) = entry.path().extension() else {
        return false;
    };
    let lower = extension.to_ascii_lowercase();
    lower == "cr2" || lower == "jpg"
}

use anyhow::anyhow;
use glib::variant::ToVariant;
use libnotify;

// Returning i32 is necessary for libnotify to treat it like a percentage.
// Returning 1/1 = 100% to start with looks ugly, avoid it.
fn progress_percentage(current: usize, total: usize) -> i32 {
    if current == 1 && total == 1 {
        0
    } else {
        (100 * current as i32) / total as i32
    }
}

pub struct Notifier {
    notification: Option<libnotify::Notification>,
}
impl Notifier {
    pub fn new(enable: bool) -> anyhow::Result<Self> {
        let notification = if enable {
            if !libnotify::is_initted() {
                libnotify::init("camera-backup").map_err(|s| anyhow!(s))?;
            }
            let n = libnotify::Notification::new("SD card loaded", "Backing up photos...", None);
            n.set_timeout(0); // Don't expire
            n.show()?;
            Some(n)
        } else {
            None
        };
        Ok(Notifier { notification })
    }

    pub fn update(&self, current: usize, total: usize) -> anyhow::Result<()> {
        if let Some(n) = &self.notification {
            let body = format!("Backing up photos [{}/{}]", current, total);
            n.update("SD card loaded", Some(body.as_str()), None)
                .map_err(|s| anyhow!(s))?;

            n.set_hint(
                "value",
                Some(progress_percentage(current, total).to_variant()),
            );
            n.show()?;
        }
        Ok(())
    }
    pub fn signoff(self) -> anyhow::Result<()> {
        if let Some(n) = &self.notification {
            n.update("SD card loaded", Some("Photos backed up"), None)
                .map_err(|s| anyhow!(s))?;
            n.set_timeout(5000);
            // Clear the progress bar, for standard-looking notification.
            n.set_hint("value", None);
            n.show()?;
            std::mem::forget(self)
        }
        Ok(())
    }
    pub fn close(&self) -> anyhow::Result<()> {
        if let Some(n) = &self.notification {
            n.close()?;
        }
        Ok(())
    }
}
impl Drop for Notifier {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

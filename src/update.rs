//! Nimbus updates itself from this repository's GitHub Releases, through
//! [Velopack](https://velopack.io).
//!
//! An installed copy checks at launch and every few hours, downloads a newer
//! release in the background, and offers "Restart to update". An update that
//! is downloaded but not restarted into is applied at the next launch by
//! `VelopackApp`. A copy that was not installed by Velopack, such as
//! `cargo run`, has no backend and never checks.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{FutureExt as _, future::BoxFuture};
use gpui_kit::{AppContext as _, Context, SharedString, Task};
use velopack::{UpdateCheck, UpdateInfo, UpdateManager, sources::AutoSource};

/// Where releases are published.
pub const REPO_URL: &str = "https://github.com/jakeklassen/nimbus";

/// How often a running copy looks for a new release. GitHub allows 60
/// unauthenticated API requests an hour per IP, so this stays well clear.
const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);

/// Finds, downloads and applies updates. Tests pass a fake.
pub trait UpdateBackend: Send + Sync + 'static {
    /// Look for a newer release and download it. Resolves to its version
    /// once it is ready to apply, or `None` when there is nothing newer.
    fn fetch(&self) -> BoxFuture<'static, anyhow::Result<Option<String>>>;

    /// Restart into the downloaded release. Exits the process on success.
    fn restart(&self) -> anyhow::Result<()>;
}

/// Velopack's update manager reading GitHub Releases.
pub struct Velopack {
    manager: UpdateManager,
    downloaded: Arc<Mutex<Option<UpdateInfo>>>,
}

impl Velopack {
    /// `None` when this copy was not installed by Velopack.
    ///
    /// `NIMBUS_UPDATE_SOURCE` points an installed copy at another feed, such as
    /// a local folder of packages, to try a release before publishing it.
    pub fn new() -> Option<Self> {
        let source = std::env::var("NIMBUS_UPDATE_SOURCE").unwrap_or_else(|_| REPO_URL.into());
        let source = AutoSource::new(&source);
        let manager = UpdateManager::new(source, None, None).ok()?;
        Some(Self {
            manager,
            downloaded: Arc::default(),
        })
    }
}

impl UpdateBackend for Velopack {
    // Velopack's calls block on the network, so the future does too. The
    // caller polls it on GPUI's background executor.
    fn fetch(&self) -> BoxFuture<'static, anyhow::Result<Option<String>>> {
        let manager = self.manager.clone();
        let downloaded = self.downloaded.clone();
        async move {
            let UpdateCheck::UpdateAvailable(update) = manager.check_for_updates()? else {
                return Ok(None);
            };
            manager.download_updates(&update, None)?;
            let version = update.TargetFullRelease.Version.clone();
            *downloaded.lock().unwrap() = Some(*update);
            Ok(Some(version))
        }
        .boxed()
    }

    fn restart(&self) -> anyhow::Result<()> {
        let update = self.downloaded.lock().unwrap().clone();
        let update = update.ok_or_else(|| anyhow::anyhow!("no update has been downloaded"))?;
        self.manager.apply_updates_and_restart(&update)?;
        Ok(())
    }
}

/// Whether a newer Nimbus is waiting.
pub struct Updater {
    backend: Option<Arc<dyn UpdateBackend>>,
    /// The version that is downloaded and ready, once there is one.
    ready: Option<SharedString>,
    _checking: Task<()>,
}

impl Updater {
    /// Start checking in the background. `None` never checks.
    pub fn new(backend: Option<Arc<dyn UpdateBackend>>, cx: &mut Context<Self>) -> Self {
        let checking = match backend.clone() {
            Some(backend) => cx.spawn(async move |this, cx| {
                loop {
                    // A failed check, offline or rate limited, waits for the next one.
                    if let Ok(Some(version)) = cx.background_spawn(backend.fetch()).await {
                        this.update(cx, |this, cx| {
                            this.ready = Some(version.into());
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    cx.background_executor().timer(CHECK_EVERY).await;
                }
            }),
            None => Task::ready(()),
        };
        Self {
            backend,
            ready: None,
            _checking: checking,
        }
    }

    /// The downloaded version, when one is ready to restart into.
    pub fn ready(&self) -> Option<&SharedString> {
        self.ready.as_ref()
    }

    /// Restart into the downloaded version. On failure the update stays
    /// downloaded, and Velopack applies it at the next launch.
    pub fn restart(&mut self) {
        if let Some(backend) = &self.backend
            && self.ready.is_some()
        {
            backend.restart().ok();
        }
    }
}

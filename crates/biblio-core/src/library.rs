use crate::{ProcessingCancel, commands::processing::PreparedCoverCache, database, services::*};
use serde::Serialize;
use std::{ops::Deref, path::PathBuf, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;

type EventSink = dyn Fn(&'static str, serde_json::Value) + Send + Sync;

pub struct ServiceState {
    pub(crate) config_dir: PathBuf,
    pub(crate) transfer_dir: PathBuf,
    _profile_lock: std::fs::File,
    pub(crate) pool: sqlx::SqlitePool,
    pub(crate) cache_access: tokio::sync::RwLock<()>,
    pub(crate) covers: PreparedCoverCache,
    pub(crate) cancel: Arc<ProcessingCancel>,
    events: Box<EventSink>,
}

pub type Services = Arc<ServiceState>;

impl ServiceState {
    pub(crate) fn emit(
        &self,
        name: &'static str,
        payload: impl Serialize,
    ) -> serde_json::Result<()> {
        (self.events)(name, serde_json::to_value(payload)?);
        Ok(())
    }
}

pub(crate) struct Queues {
    pub upload: UnboundedSender<upload_worker::UploadJob>,
    pub download: download_worker::DownloadQueue,
    pub delete: UnboundedSender<delete_worker::DeleteJob>,
    pub reencrypt: UnboundedSender<reencrypt_worker::ReencryptJob>,
    pub import: UnboundedSender<import_worker::ImportJob>,
}

/// Both desktop shells call these services. Workers retain only ServiceState so
/// dropping the last Library closes their queues.
#[derive(Clone)]
pub struct Library {
    services: Services,
    pub(crate) queues: Arc<Queues>,
}

impl Deref for Library {
    type Target = Services;
    fn deref(&self) -> &Services {
        &self.services
    }
}

impl Library {
    pub async fn open(
        config_dir: PathBuf,
        events: impl Fn(&'static str, serde_json::Value) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let transfer_dir = config_dir.join("transfer-work");
        std::fs::create_dir_all(&transfer_dir).map_err(|e| e.to_string())?;
        let profile_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(transfer_dir.join("library.lock"))
            .map_err(|e| e.to_string())?;
        profile_lock.try_lock().map_err(|e| {
            format!(
                "Cannot acquire the library lock at {}: {e}",
                config_dir.display()
            )
        })?;
        let pool = database::open(&config_dir).await?;
        upload_worker::sweep_stale_upload_temps(&transfer_dir);
        let services = Arc::new(ServiceState {
            pool,
            cache_access: Default::default(),
            config_dir,
            transfer_dir,
            _profile_lock: profile_lock,
            events: Box::new(events),
            covers: PreparedCoverCache::new(),
            cancel: Arc::new(ProcessingCancel::new()),
        });
        let queues = Arc::new(Queues {
            import: import_worker::spawn(services.clone()),
            upload: upload_worker::spawn(services.clone()),
            download: download_worker::spawn(services.clone()),
            delete: delete_worker::spawn(services.clone()),
            reencrypt: reencrypt_worker::spawn(services.clone()),
        });
        Ok(Self { services, queues })
    }
}

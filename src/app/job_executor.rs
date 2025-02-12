use std::{collections::BTreeMap, sync::Arc};

use egui::{mutex::RwLock, Ui};
use log::debug;

use super::AnnatomicApp;

/// A job during no UI interaction should be possible. The job is run in a
/// different background thread so we can inform the use about the progress and
/// the app does not freeze. But the user should not be able to make any
/// meaningful changes.
#[derive(Clone, Default)]
pub(crate) struct FgJob {
    msg: Arc<RwLock<Option<String>>>,
}

impl FgJob {
    pub(crate) fn update_message<S>(&self, message: S)
    where
        S: Into<String>,
    {
        let mut lock = self.msg.write();
        lock.replace(message.into());
    }
}

type FnStateUpdate = Box<dyn FnOnce(&mut AnnatomicApp) + Send + Sync>;

#[derive(Default, Clone)]
pub(crate) struct JobExecutor {
    running: Arc<RwLock<BTreeMap<String, FgJob>>>,
    finished: Arc<RwLock<BTreeMap<String, FnStateUpdate>>>,
    failed: Arc<RwLock<BTreeMap<String, anyhow::Error>>>,
}

impl JobExecutor {
    pub(crate) fn add<F, U, R>(&self, title: &str, worker: F, state_updater: U)
    where
        F: FnOnce(FgJob) -> anyhow::Result<R> + Send + 'static,
        U: FnOnce(R, &mut AnnatomicApp) + Send + Sync + 'static,
        R: Send + Sync + 'static,
    {
        debug!("Adding foreground job \"{title}\"");
        let running_jobs = self.running.clone();
        let failed_jobs = self.failed.clone();
        let finished_jobs = self.finished.clone();

        let single_job = FgJob::default();
        {
            let mut lock = running_jobs.write();
            lock.insert(title.to_string(), single_job.clone());
            debug!("Number of currently running jobs: {}", lock.len());
        }
        let title = title.to_string();
        rayon::spawn(move || {
            debug!("Spawning foreground job \"{title}\"");
            let result = worker(single_job);
            debug!("Finished foreground job \"{title}\"");
            match result {
                Ok(result) => {
                    let mut finished_jobs = finished_jobs.write();
                    finished_jobs.insert(
                        title.clone(),
                        Box::new(move |app| state_updater(result, app)),
                    );
                }
                Err(err) => {
                    let mut failed_jobs = failed_jobs.write();
                    failed_jobs.insert(title.clone(), err);
                }
            }
            let mut jobs = running_jobs.write();
            jobs.remove(&title);
        });
    }

    pub(super) fn show(&self, ui: &mut Ui, app: &mut AnnatomicApp) -> bool {
        let mut failed_jobs = self.failed.write();
        while let Some((_title, e)) = failed_jobs.pop_first() {
            app.notifier.report_error(e);
        }

        let mut finished_jobs = self.finished.write();
        while let Some(j) = finished_jobs.pop_first() {
            j.1(app);
        }

        let running_jobs = self.running.read();
        let has_jobs = !running_jobs.is_empty();
        for (title, job) in running_jobs.iter() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.heading(title);
            });

            let msg = job.msg.read();
            ui.label(
                msg.clone()
                    .unwrap_or_else(|| "Please wait for the background job to finish".into()),
            );
        }

        has_jobs
    }

    pub(crate) fn has_active_job_with_title(&self, title: &str) -> bool {
        let running_jobs = self.running.read();
        running_jobs.contains_key(title)
    }

    #[cfg(test)]
    pub(crate) fn has_running_jobs(&self) -> bool {
        let running_jobs = self.running.read();
        !running_jobs.is_empty()
    }
}

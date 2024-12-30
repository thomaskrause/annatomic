use std::sync::Mutex;

use anyhow::Error;
use crossbeam_queue::SegQueue;
use egui::Context;
use egui_notify::{Toast, Toasts};
use log::error;

#[derive(Default)]
pub(crate) struct Notifier {
    toasts: Mutex<Toasts>,
    error_queue: SegQueue<Error>,
}

impl Notifier {
    pub(crate) fn report_error(&self, err: Error) {
        if err.chain().len() > 1 {
            error!("{err}: {}", err.root_cause().to_string());
        } else {
            error!("{err}");
        }
        self.error_queue.push(err);
    }

    pub(crate) fn unwrap_or_default<T>(&self, result: anyhow::Result<T>) -> T
    where
        T: Default,
    {
        match result {
            Ok(o) => o,
            Err(e) => {
                self.report_error(e);
                T::default()
            }
        }
    }

    pub(crate) fn add_toast(&self, toast: Toast) {
        let messages = self.toasts.lock();
        match messages {
            Ok(mut messages) => {
                messages.add(toast);
            }
            Err(lock_error) => {
                error!("Error trying to report internal error: {lock_error}");
            }
        }
    }
    pub(super) fn show(&self, ctx: &Context) {
        let messages = self.toasts.lock();
        match messages {
            Ok(mut messages) => {
                while let Some(e) = self.error_queue.pop() {
                    let error_msg = if e.chain().len() > 1 {
                        format!("{e}: {}", e.root_cause())
                    } else {
                        format!("{e}")
                    };
                    messages.error(error_msg);
                }
                messages.show(ctx);
            }
            Err(lock_error) => {
                error!("Error trying to report internal error: {lock_error}");
            }
        }
    }
}

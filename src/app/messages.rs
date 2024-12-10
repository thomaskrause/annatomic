use std::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};

use anyhow::Error;
use egui::Context;
use egui_notify::{Toast, Toasts};
use log::error;

pub(crate) struct Notifier {
    messages: Mutex<Toasts>,
    tx: Sender<Error>,
    rx: Receiver<Error>,
}

impl Default for Notifier {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Error>();
        Self {
            tx,
            rx,
            messages: Default::default(),
        }
    }
}

impl Notifier {
    pub(crate) fn handle_error(&self, err: Error) {
        error!("{err}");
        if let Err(e) = self.tx.send(err) {
            error!("Error trying to create internal error representation: {e}");
        }
    }
    pub(crate) fn add_toast(&self, toast: Toast) {
        let messages = self.messages.lock();
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
        let messages = self.messages.lock();
        match messages {
            Ok(mut messages) => {
                match self.rx.try_recv() {
                    Ok(msg) => {
                        messages.error(msg.to_string());
                    }
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => {}
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            error!("Error notification channel disconnected")
                        }
                    },
                }
                messages.show(ctx);
            }
            Err(lock_error) => {
                error!("Error trying to report internal error: {lock_error}");
            }
        }
    }
}

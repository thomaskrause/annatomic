use std::{io::Read, path::PathBuf};

use egui::{mutex::RwLock, Context, Id};
use egui_kittest::{kittest::Queryable, Harness};
use graphannis::model::AnnotationComponentType;
use tempfile::TempDir;

use super::*;

/// 30 Seconds, since th tests are run with 4fps
pub const MAX_WAIT_STEPS: usize = 120;

pub(crate) fn create_app_with_corpus<R: Read, S: Into<String>>(
    corpus_name: S,
    graphml: R,
) -> crate::AnnatomicApp {
    // Load the graphml into a temporary folder
    let (mut graph, _config) = graphannis_core::graph::serialization::graphml::import::<
        AnnotationComponentType,
        _,
        _,
    >(graphml, false, |_| {})
    .unwrap();
    let graph_dir = TempDir::new().unwrap();
    graph.persist_to(graph_dir.path()).unwrap();

    let mut app_state = crate::AnnatomicApp::default();
    app_state
        .project
        .corpus_locations
        .insert(corpus_name.into(), graph_dir.into_path());
    app_state
}

pub(crate) fn create_test_harness(
    app_state: crate::AnnatomicApp,
) -> (Harness<'static>, Arc<RwLock<crate::AnnatomicApp>>) {
    let app_state = Arc::new(RwLock::new(app_state));
    let result_app_state = app_state.clone();
    let app = move |ctx: &Context| {
        let frame_info = IntegrationInfo {
            cpu_usage: Some(3.14),
        };
        let mut app_state = app_state.write();
        set_fonts(ctx);
        app_state.show(ctx, &frame_info);
    };

    let harness = Harness::builder()
        .with_size(egui::Vec2::new(800.0, 600.0))
        .with_max_steps(24)
        .build(app);

    (harness, result_app_state.clone())
}

pub(crate) fn wait_for_editor(
    harness: &mut Harness<'static>,
    app_state: Arc<RwLock<crate::AnnatomicApp>>,
) {
    // Wait until editor has been created
    for i in 0..MAX_WAIT_STEPS {
        harness.step();
        let app_state = app_state.read();
        if i > 3 && app_state.current_editor.get().is_some() {
            break;
        }
    }

    wait_until_jobs_finished(harness, app_state);
}

pub(crate) fn focus_and_wait(harness: &mut Harness<'static>, id: Id) {
    harness.get_by(|n| n.id().0 == id.value()).focus();
    for i in 0..MAX_WAIT_STEPS {
        harness.step();
        if i > 3 && harness.get_by(|n| n.id().0 == id.value()).is_focused() {
            break;
        }
    }
    harness.step();
}

pub(crate) fn wait_for_editor_vanished(
    harness: &mut Harness<'static>,
    app_state: Arc<RwLock<crate::AnnatomicApp>>,
) {
    for i in 0..MAX_WAIT_STEPS {
        harness.step();
        let app_state = app_state.read();
        if i > 3 && app_state.current_editor.get().is_none() {
            break;
        }
    }

    wait_until_jobs_finished(harness, app_state);
}

pub(crate) fn wait_until_jobs_finished(
    harness: &mut Harness<'static>,
    app_state: Arc<RwLock<crate::AnnatomicApp>>,
) {
    let mut steps_without_jobs = 0;
    for _ in 0..MAX_WAIT_STEPS {
        harness.step();
        let app_state = app_state.read();
        if !app_state.jobs.has_running_jobs() {
            steps_without_jobs += 1;
        } else {
            steps_without_jobs = 0;
        }

        if steps_without_jobs > 10 {
            break;
        }
    }
    // Jobs can create notifications wait until these are finished, too
    wait_until_notifications_finished(harness, app_state);
}

pub(crate) fn wait_until_notifications_finished(
    harness: &mut Harness<'static>,
    app_state: Arc<RwLock<crate::AnnatomicApp>>,
) {
    let mut steps_without_notification = 0;
    for _ in 0..MAX_WAIT_STEPS {
        harness.step();
        let app_state = app_state.read();
        if !app_state.notifier.is_empty() {
            steps_without_notification += 1;
        } else {
            steps_without_notification = 0;
        }

        if steps_without_notification > 10 {
            break;
        }
    }
    harness.step();
}

#[macro_export]
macro_rules! assert_screenshots {
    ($($x:expr),* ) => {
        $(
            match $x {
                Ok(_) => {}
                Err(err) => {
                    panic!("{}", err);
                }
            }
        )*
    };
}

#[test]
fn show_main_page() {
    let mut app_state = crate::AnnatomicApp::default();

    app_state
        .project
        .corpus_locations
        .insert("single_sentence".to_string(), PathBuf::default());
    app_state
        .project
        .corpus_locations
        .insert("test".to_string(), PathBuf::default());

    let (mut harness, _) = create_test_harness(app_state);
    harness.run();

    harness.snapshot("show_main_page");
}

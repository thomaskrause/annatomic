use crate::app::tests::{create_app_with_corpus, create_test_harness, wait_for_corpus_tree};
use egui::{accesskit::Role, Id};
use egui_kittest::kittest::Queryable;

#[test]
fn select_corpus() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    harness.get_by_label("single_sentence").click();
    wait_for_corpus_tree(&mut harness, app_state.clone());

    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_some());
        assert_eq!(
            "single_sentence",
            app_state.project.selected_corpus.as_ref().unwrap().name
        );
        assert!(app_state.corpus_tree.is_some());
    }

    harness.wgpu_snapshot("select_corpus");
}

#[test]
fn create_new_corpus() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    let inputs: Vec<_> = harness
        .get_all_by_role(Role::TextInput)
        .filter(|t| t.id().0 == Id::from("new-corpus-name").value())
        .collect();
    inputs[0].type_text("example");
    harness.get_by_label("Add").click();

    for i in 0..10_000 {
        harness.step();
        let app_state = app_state.read();
        if i > 10 && app_state.corpus_tree.is_some() && app_state.notifier.is_empty() {
            break;
        }
    }
    // Add some runs, so that the toasts have time to disappear
    for _ in 0..10 {
        harness.step();
    }

    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_some());
        assert_eq!(
            "example",
            app_state.project.selected_corpus.as_ref().unwrap().name
        );
        assert!(app_state.corpus_tree.is_some());
    }

    harness.wgpu_snapshot("create_new_corpus");
}

#[test]
fn delete_corpus() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();
    harness.get_by_label("single_sentence").click();
    {
        // Programmatically mark the corpus for deletion
        let mut app_state = app_state.write();
        app_state.project.scheduled_for_deletion = Some("single_sentence".to_string());
    }
    for i in 0..10_000 {
        harness.step();
        let app_state = app_state.read();
        if i > 10 && app_state.corpus_tree.is_none() {
            break;
        }
    }
    let confirmation_result = harness.try_wgpu_snapshot("delete_corpus_confirmation");

    harness.get_by_label_contains("Delete").click();
    harness.run();
    for _ in 0..10_000 {
        harness.step();
        let app_state = app_state.read();
        if !app_state.jobs.has_running_jobs() && app_state.corpus_tree.is_none() {
            break;
        }
    }
    harness.run();
    let final_result = harness.try_wgpu_snapshot("delete_corpus");
    assert!(confirmation_result.is_ok());
    assert!(final_result.is_ok());
    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_none());
        assert!(app_state.corpus_tree.is_none());
    }
}
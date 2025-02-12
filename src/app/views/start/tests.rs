use crate::{
    app::tests::{
        create_app_with_corpus, create_test_harness, wait_for_editor, wait_for_editor_vanished,
    },
    assert_screenshots,
};
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
    wait_for_editor(&mut harness, app_state.clone());

    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_some());
        assert_eq!(
            "single_sentence",
            app_state.project.selected_corpus.as_ref().unwrap().name
        );
        assert!(app_state.current_editor.get().is_some());
    }

    harness.snapshot("select_corpus");
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

    for i in 0..120 {
        harness.step();
        let app_state = app_state.read();
        if i > 10 && app_state.current_editor.get().is_some() && app_state.notifier.is_empty() {
            break;
        }
    }
    harness.run();

    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_some());
        assert_eq!(
            "example",
            app_state.project.selected_corpus.as_ref().unwrap().name
        );
        assert!(app_state.current_editor.get().is_some());
    }

    harness.snapshot("create_new_corpus");
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
    wait_for_editor(&mut harness, app_state.clone());
    let confirmation_result = harness.try_snapshot("delete_corpus_confirmation");

    harness.get_by_label_contains("Delete").click();
    harness.step();
    wait_for_editor_vanished(&mut harness, app_state.clone());
    let final_result = harness.try_snapshot("delete_corpus");
    assert_screenshots!(confirmation_result, final_result);
    {
        let app_state = app_state.read();
        assert!(app_state.project.selected_corpus.is_none());
        assert!(app_state.current_editor.get().is_none());
    }
}

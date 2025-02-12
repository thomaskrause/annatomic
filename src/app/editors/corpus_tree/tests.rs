use std::sync::Arc;

use crate::{
    app::tests::{
        create_app_with_corpus, create_test_harness, focus_and_wait, wait_for_editor,
        wait_until_jobs_finished,
    },
    assert_screenshots,
};
use egui::{accesskit::Role, mutex::RwLock, Id};
use egui_kittest::{
    kittest::{Key, Queryable},
    Harness,
};
use egui_phosphor::regular::{PLUS_CIRCLE, TRASH};
use graphannis::aql;

#[test]
fn show_metadata() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    // Select the corpus and the document
    harness.get_by_label("single_sentence").click();
    wait_for_editor(&mut harness, app_state.clone());
    harness.get_by_label("single_sentence/zossen").click();

    harness.run();
    harness.snapshot("show_metadata");
}

fn save_pending_changes_action(
    harness: &mut Harness<'static>,
    app_state: Arc<RwLock<crate::AnnatomicApp>>,
) {
    harness.run();
    {
        let mut app_state = app_state.write();
        app_state.apply_pending_updates();
    }
    wait_until_jobs_finished(harness, app_state.clone());
    wait_for_editor(harness, app_state);
}

fn query_count(query: &str, app_state: Arc<RwLock<crate::AnnatomicApp>>) -> usize {
    let app_state = app_state.read();

    let graph = app_state.project.get_selected_graph().unwrap().unwrap();

    let query = aql::parse(query, false).unwrap();
    let count = aql::execute_query_on_graph(&graph.read(), &query, true, None)
        .unwrap()
        .count();
    count
}

#[test]
fn undo_redo() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    // Select the corpus and the document
    harness.get_by_label("single_sentence").click();
    wait_for_editor(&mut harness, app_state.clone());
    harness.get_by_label("single_sentence/zossen").click();
    harness.run();

    // Edit the node name twice
    harness
        .get_by(|n| n.role() == Role::TextInput && n.value().unwrap_or_default() == "zossen")
        .type_text("-1");
    save_pending_changes_action(&mut harness, app_state.clone());

    assert_eq!(0, query_count("annis:doc=\"zossen\"", app_state.clone()));
    assert_eq!(1, query_count("annis:doc=\"zossen-1\"", app_state.clone()));
    let r1 = harness.try_snapshot("undo_redo_1");

    let text_input = harness
        .get_by(|n| n.role() == Role::TextInput && n.value().unwrap_or_default() == "zossen-1");
    text_input.press_keys(&[Key::Backspace]);
    text_input.type_text("2");
    save_pending_changes_action(&mut harness, app_state.clone());

    assert_eq!(1, query_count("annis:doc=\"zossen-2\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen-1\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen\"", app_state.clone()));
    let r2 = harness.try_snapshot("undo_redo_2");

    // Undo last change
    {
        let mut app_state = app_state.write();
        app_state.project.undo();
    }
    wait_for_editor(&mut harness, app_state.clone());

    assert_eq!(1, query_count("annis:doc=\"zossen-1\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen-2\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen\"", app_state.clone()));
    let r3 = harness.try_snapshot("undo_redo_3");

    // Redo, so the name should be "zossen-2" again
    {
        let mut app_state = app_state.write();
        app_state.project.redo();
    }
    wait_for_editor(&mut harness, app_state.clone());

    assert_eq!(1, query_count("annis:doc=\"zossen-2\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen-1\"", app_state.clone()));
    assert_eq!(0, query_count("annis:doc=\"zossen\"", app_state.clone()));
    let r4 = harness.try_snapshot("undo_redo_4");

    assert_screenshots![r1, r2, r3, r4];
}

#[test]
fn add_and_delete_entry() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    // Select the corpus and the document
    harness.get_by_label("single_sentence").click();
    wait_for_editor(&mut harness, app_state.clone());
    harness.get_by_label("single_sentence/zossen").click();
    harness.run();

    wait_for_editor(&mut harness, app_state.clone());

    let namespace_id = Id::new("new-metadata-entry-ns");
    focus_and_wait(&mut harness, namespace_id);
    harness
        .get_by(|n| n.id().0 == namespace_id.value())
        .type_text("test");
    harness.run();

    let name_id = Id::new("new-metadata-entry-name");
    focus_and_wait(&mut harness, name_id);
    harness
        .get_by(|n| n.id().0 == name_id.value())
        .type_text("example");
    harness.run();

    let value_id = Id::new("new-metadata-entry-value");
    focus_and_wait(&mut harness, value_id);
    // Fill out the the value text field
    let text_value = harness
        .get_all_by_role(Role::TextInput)
        .filter(|t| t.id().0 == value_id.value())
        .next()
        .unwrap();
    text_value.type_text("example-value");
    harness.run();

    harness.get_by_label(PLUS_CIRCLE).click();

    wait_for_editor(&mut harness, app_state.clone());

    let r1 = harness.try_snapshot("after-adding-metadata");

    // Delete the entry again
    let delete_buttons: Vec<_> = harness.get_all_by_label(TRASH).collect();
    delete_buttons[3].click();
    wait_for_editor(&mut harness, app_state.clone());

    let r2 = harness.try_snapshot("after-deleting-metadata");

    assert_screenshots![r1, r2];
}

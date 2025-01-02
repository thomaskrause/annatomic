use crate::{
    app::tests::{
        create_app_with_corpus, create_test_harness, wait_for_corpus_tree, wait_until_jobs_finished,
    },
    assert_snapshots,
};
use egui::accesskit::Role;
use egui_kittest::kittest::{Key, Queryable};
use graphannis::aql;

#[test]
fn show_metadata() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    // Select the corpus and the document
    harness.get_by_label("single_sentence").click();
    wait_for_corpus_tree(&mut harness, app_state.clone());
    harness.get_by_label("single_sentence/zossen").click();

    harness.run();
    harness.wgpu_snapshot("show_metadata");
}

#[test]
fn undo_redo() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../tests/data/single_sentence.graphml")[..],
    );
    let (mut harness, app_state) = create_test_harness(app_state);
    harness.run();

    // Select the corpus and the document
    harness.get_by_label("single_sentence").click();
    wait_for_corpus_tree(&mut harness, app_state.clone());
    harness.get_by_label("single_sentence/zossen").click();
    harness.run();

    // Edit the node name twice
    harness
        .get_by(|n| n.role() == Role::TextInput && n.value().unwrap_or_default() == "zossen")
        .type_text("-1");

    harness
        .get_by_role_and_label(Role::Button, "Apply document updates")
        .click();
    wait_until_jobs_finished(&mut harness, app_state.clone());
    let r1 = harness.try_wgpu_snapshot("undo_redo_1");
    let text_input = harness
        .get_by(|n| n.role() == Role::TextInput && n.value().unwrap_or_default() == "zossen-1");
    text_input.press_keys(&[Key::Backspace]);
    text_input.type_text("2");

    harness
        .get_by_role_and_label(Role::Button, "Apply document updates")
        .click();
    wait_until_jobs_finished(&mut harness, app_state.clone());
    let r2 = harness.try_wgpu_snapshot("undo_redo_2");

    // Undo last change
    {
        let mut app_state = app_state.write();
        let mut jobs = app_state.jobs.clone();
        app_state.project.undo(&mut jobs);
    }
    wait_until_jobs_finished(&mut harness, app_state.clone());
    let r3 = harness.try_wgpu_snapshot("undo_redo_3");

    {
        let app_state = app_state.read();

        let graph = app_state.project.get_selected_graph().unwrap().unwrap();

        let query = aql::parse("annis:doc=\"zossen-1\"", false).unwrap();
        let count = aql::execute_query_on_graph(&graph.read(), &query, true, None)
            .unwrap()
            .count();
        assert_eq!(1, count);
    }

    // Redo, so the name should be "zossen-2" again
    {
        let mut app_state = app_state.write();
        let mut jobs = app_state.jobs.clone();
        app_state.project.redo(&mut jobs);
    }
    wait_until_jobs_finished(&mut harness, app_state.clone());
    let r4 = harness.try_wgpu_snapshot("undo_redo_4");
    assert_snapshots![r1, r2, r3, r4];

    {
        let app_state = app_state.read();

        let graph = app_state.project.get_selected_graph().unwrap().unwrap();
        let query = aql::parse("annis:doc=\"zossen-2\"", false).unwrap();
        let count = aql::execute_query_on_graph(&graph.read(), &query, true, None)
            .unwrap()
            .count();
        assert_eq!(1, count);
    }
}

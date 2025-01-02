use crate::app::tests::{create_app_with_corpus, create_test_harness, wait_for_corpus_tree};
use egui_kittest::kittest::Queryable;

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

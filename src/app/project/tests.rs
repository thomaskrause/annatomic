use insta::assert_snapshot;
use tempfile::NamedTempFile;

use crate::app::tests::{create_app_with_corpus, create_test_harness, wait_until_jobs_finished};

#[test]
fn export_corpus() {
    let app_state = create_app_with_corpus(
        "single_sentence",
        &include_bytes!("../../../tests/data/single_sentence.graphml")[..],
    );
    let export_location = NamedTempFile::new().unwrap();

    let (mut harness, app_state) = create_test_harness(app_state);
    {
        // Select the corpus and export it
        let mut app_state = app_state.write();
        app_state
            .project
            .select_corpus(Some("single_sentence".to_string()));

        app_state.project.export_to_graphml(export_location.path());
    }

    // Execute the running jobs and check that the file has been created
    wait_until_jobs_finished(&mut harness, app_state.clone());

    let actual_graphml = std::fs::read_to_string(export_location.path()).unwrap();
    assert_snapshot!(actual_graphml);
}

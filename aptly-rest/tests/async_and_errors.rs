use aptly_rest::{AptlyRest, AptlyRestError, TaskOr};
use aptly_rest_mock::AptlyRestMock;

#[tokio::test]
async fn db_cleanup_sync() {
    let mock = AptlyRestMock::start().await;
    let aptly = AptlyRest::new(mock.url());

    let result = aptly.db_cleanup().await.unwrap();
    assert!(matches!(result, TaskOr::Value(_)));
}

#[tokio::test]
async fn db_cleanup_async() {
    let mock = AptlyRestMock::start().await;
    let mut aptly = AptlyRest::new(mock.url());
    aptly.async_mode = true;

    let result = aptly.db_cleanup().await.unwrap();
    match result {
        TaskOr::Task(task) => {
            assert_eq!(task.name, "Clean up db");
        }
        TaskOr::Value(value) => panic!("Expected Task, got {:?}", value),
    }
}

#[tokio::test]
async fn db_cleanup_conflict() {
    let mock = AptlyRestMock::start().await;
    let mut aptly = AptlyRest::new(mock.url());
    aptly.async_mode = true;

    // First call succeeds with a task
    let result = aptly.db_cleanup().await.unwrap();
    assert!(matches!(result, TaskOr::Task(_)));

    // Second call hits 409 because the first is still running
    let err = aptly.db_cleanup().await.unwrap_err();
    match err {
        AptlyRestError::AptlyError { status, message } => {
            assert_eq!(status, 409);
            assert_eq!(message, "Needed resources are used by other tasks.");
        }
        other => panic!("Expected AptlyError, got {:?}", other),
    }
}

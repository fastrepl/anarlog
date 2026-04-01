use cactus::Model;

// cargo test -p cactus --test vad test_invalid_model_path -- --nocapture
#[test]
fn test_invalid_model_path() {
    let r = Model::new("/nonexistent/path/to/model");
    assert!(r.is_err());
}

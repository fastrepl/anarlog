use std::future::Future;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub trait HttpClient: Send + Sync {
    fn get(&self, path: &str) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;

    fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;

    fn put(&self, path: &str, body: Vec<u8>)
    -> impl Future<Output = Result<Vec<u8>, Error>> + Send;

    fn patch(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;

    fn delete(&self, path: &str) -> impl Future<Output = Result<Vec<u8>, Error>> + Send;
}

use futures_util::{Stream, StreamExt};
use tokio::io::AsyncWriteExt;

pub async fn save(
    stream: impl Stream<Item = f32>,
    local_path: std::path::PathBuf,
) -> Result<(), crate::Error> {
    let mut file = tokio::fs::File::create(local_path).await?;
    let mut stream = std::pin::pin!(stream);

    while let Some(value) = stream.next().await {
        file.write_all(&value.to_le_bytes()).await?;
    }

    file.flush().await?;
    Ok(())
}

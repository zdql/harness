use super::progress::ProgressReporter;
use super::protocol::{format_job, preview};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

pub(crate) async fn read_logged_stream<R>(
    provider: &'static str,
    stream_name: &'static str,
    job_id: String,
    reader: R,
    progress: Option<ProgressReporter>,
) -> Result<Vec<u8>, String>
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut collected = Vec::new();
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|e| format!("failed to read {provider} {stream_name}: {e}"))?;
        if bytes == 0 {
            break;
        }
        collected.extend_from_slice(&line);
        let text = String::from_utf8_lossy(&line);
        eprintln!(
            "{provider} {stream_name}{} bytes={} preview={}",
            format_job(Some(&job_id)),
            line.len(),
            preview(&text)
        );
        if let Some(reporter) = progress.as_ref() {
            reporter.push(&format!("{provider} {stream_name}: {}", text.trim()));
        }
    }

    Ok(collected)
}

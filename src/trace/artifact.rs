use std::io::{self, Read};

const MAX_STDIN_BYTES: usize = 16 * 1024 * 1024;

/// Read one source-free kernel artifact from a file or bounded stdin. This is
/// deliberately the only consumer-side loader: validation and deserialization
/// remain owned by `KernelArtifact::from_json`.
pub fn read_kernel_artifact(
    path: &std::path::Path,
) -> Result<crate::KernelArtifact, String> {
    let json = if path == std::path::Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin()
            .lock()
            .take(MAX_STDIN_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read kernel artifact JSON from stdin: {error}"))?;
        if bytes.len() > MAX_STDIN_BYTES {
            return Err(format!(
                "kernel artifact JSON from stdin exceeds {MAX_STDIN_BYTES}-byte limit"
            ));
        }
        String::from_utf8(bytes)
            .map_err(|error| format!("kernel artifact JSON from stdin is not UTF-8: {error}"))?
    } else {
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if metadata.len() > crate::MAX_ARTIFACT_BYTES as u64 {
            return Err(format!(
                "{} is {} bytes, over the {}-byte artifact limit",
                path.display(),
                metadata.len(),
                crate::MAX_ARTIFACT_BYTES
            ));
        }
        std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?
    };
    crate::KernelArtifact::from_json(&json).map_err(|error| {
        let detail = error.to_string();
        if detail.contains("shape-pipeline-report/1") || detail.contains("saved report") {
            format!(
                "{}: legacy PipelineReport is not a runtime kernel artifact; regenerate from raw source with `shape deob SOURCE --report ARTIFACT`",
                path.display()
            )
        } else {
            format!("{}: {detail}", path.display())
        }
    })
}

//! Portable, synchronous filesystem services for authored MMS tools.
use super::runtime_config::{MittensApi, MittensBinding};
use meow_meow_script as mms;

pub(super) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    builder.namespace("File", |namespace| {
        namespace.api(
            "read_text",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::String),
            MittensBinding::Api(MittensApi::FileReadText),
        );
        namespace.api(
            "list_dir",
            mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Array),
            MittensBinding::Api(MittensApi::FileListDir),
        );
    });
}

pub(super) fn read_text(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    let [mms::TransportValue::String(path)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "File.read_text",
            "expected one string path argument",
        ));
    };
    let text = std::fs::read_to_string(path).map_err(|error| {
        mms::HostError::failure("File.read_text", format!("cannot read '{path}': {error}"))
    })?;
    Ok(mms::HostResponse::Transport(mms::TransportValue::String(
        text,
    )))
}

/// One directory, directories first then UTF-8 name order. Symlinks are
/// reported without following them. Paths use the caller's original root.
pub(super) fn list_dir(
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    use mms::TransportValue as V;
    let [V::String(path)] = args.as_slice() else {
        return Err(mms::HostError::failure(
            "File.list_dir",
            "expected one string path argument",
        ));
    };
    let fail = |error: String| {
        mms::HostError::failure("File.list_dir", format!("cannot list '{path}': {error}"))
    };
    let directory = std::fs::read_dir(path).map_err(|e| fail(e.to_string()))?;
    let mut entries = Vec::new();
    for entry in directory {
        if entries.len() == 4096 {
            return Err(fail("directory exceeds 4096 entries".into()));
        }
        let entry = entry.map_err(|e| fail(e.to_string()))?;
        let kind = entry.file_type().map_err(|e| fail(e.to_string()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| fail("non-UTF8 entry name".into()))?;
        let entry_path = entry.path();
        let entry_path = entry_path
            .to_str()
            .ok_or_else(|| fail("non-UTF8 entry path".into()))?;
        let extension = std::path::Path::new(&name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        entries.push((
            !kind.is_dir(),
            name.clone(),
            V::Table(Vec::from([
                ("name".into(), V::String(name)),
                ("path".into(), V::String(entry_path.into())),
                ("extension".into(), V::String(extension)),
                ("is_dir".into(), V::Bool(kind.is_dir())),
                ("is_file".into(), V::Bool(kind.is_file())),
                ("is_symlink".into(), V::Bool(kind.is_symlink())),
            ])),
        ));
    }
    entries.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    Ok(mms::HostResponse::Transport(V::Array(
        entries.into_iter().map(|(_, _, entry)| entry).collect(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_listing_exposes_real_files_and_reports_missing_directories() {
        let mms::HostResponse::Transport(mms::TransportValue::Array(entries)) =
            list_dir(vec![mms::TransportValue::String(
                "examples/mitten-droid".into(),
            )])
            .unwrap()
        else {
            panic!("expected entries")
        };
        assert!(entries.iter().any(|entry| matches!(entry, mms::TransportValue::Table(fields) if fields.iter().any(|(key,value)| key == "name" && value == &mms::TransportValue::String("hello.mms".into())))));
        assert!(
            list_dir(vec![mms::TransportValue::String(
                "examples/mitten-droid/missing".into()
            )])
            .is_err()
        );
    }
}

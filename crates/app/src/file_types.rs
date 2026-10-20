use std::io;

pub(crate) struct SupportedFileType {
    pub(crate) category: String,
    pub(crate) extensions: Vec<String>,
}

pub(crate) fn load() -> io::Result<Vec<SupportedFileType>> {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/default-file-types.csv"
    ))
    .lines()
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
    .map(|line| {
        let mut fields = line.split('|');
        let category = fields.next();
        let _identifier = fields.next();
        let extensions = fields.next();
        if fields.next().is_some() {
            return Err(invalid_manifest());
        }
        match (category, _identifier, extensions) {
            (Some(category), Some(identifier), Some(extensions))
                if matches!(category, "video" | "audio" | "playlist")
                    && !identifier.is_empty()
                    && !extensions.is_empty() =>
            {
                let extensions = extensions.split(',').map(str::to_owned).collect();
                Ok(SupportedFileType {
                    category: category.to_owned(),
                    extensions,
                })
            }
            _ => Err(invalid_manifest()),
        }
    })
    .collect()
}

fn invalid_manifest() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "the default file type manifest contains an invalid row",
    )
}

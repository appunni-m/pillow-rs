    /// Read a logical mode only when metadata establishes it without decoding
    /// or executing pixels. Unknown contracts remain execution-time checks.
    pub(crate) fn known_mode(&self) -> Option<String> {
        match self {
            Self::Loaded(_) | Self::Paletted(_) | Self::Bytes { info: Some(_), .. } => {
                self.mode().ok()
            }
            Self::Pipeline {
                explicit_mode: Some(mode),
                ..
            } => Some(mode.clone()),
            Self::Pipeline { source, ops, .. } => {
                let mut current = source.known_mode()?;
                for operation in ops.iter() {
                    current = known_pipeline_op_mode(operation, &current)?;
                }
                Some(current)
            }
            _ => None,
        }
    }

    /// Read dimensions only when metadata establishes them without pixels.
    pub(crate) fn known_size(&self) -> Option<(u32, u32)> {
        match self {
            Self::Loaded(data) => Some(data.image.dimensions()),
            Self::Paletted(data) => Some(data.indices.dimensions()),
            Self::Bytes {
                info: Some(info), ..
            } => Some((info.width, info.height)),
            Self::Pipeline { source, ops, .. } => {
                let mut size = source.known_size()?;
                for operation in ops.iter() {
                    size = known_pipeline_op_dimensions(operation, size)?;
                }
                Some(size)
            }
            _ => None,
        }
    }


    /// Seeds an ordinary operation result with pixels produced by the
    /// explicit batch executor. The lazy result still owns the same source,
    /// mode, palette, and metadata path as its single-image equivalent.
    pub(crate) fn cache_batched_materialization(
        &mut self,
        image: Arc<DynamicImage>,
    ) -> Result<(), PilError> {
        let Image::Pipeline { materialized, .. } = self else {
            return Err(PilError::InternalError(
                "batched pixels require an operation pipeline result".into(),
            ));
        };
        materialized
            .set(Ok(image))
            .map_err(|_| PilError::InternalError("batch result was already materialized".into()))
    }

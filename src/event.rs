use crate::{
    Encoding, MappingStyle, Mark, ScalarStyle, SequenceStyle, TagDirective, VersionDirective,
};

/// The event structure.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Event {
    /// The event data.
    pub data: EventData,
    /// The beginning of the event.
    pub start_mark: Mark,
    /// The end of the event.
    pub end_mark: Mark,
}

#[derive(Debug, PartialEq)]
pub enum EventData {
    /// The stream parameters (for `YAML_STREAM_START_EVENT`).
    StreamStart {
        /// The document encoding.
        encoding: Encoding,
    },
    StreamEnd,
    /// The document parameters (for `YAML_DOCUMENT_START_EVENT`).
    DocumentStart {
        /// The version directive.
        version_directive: Option<VersionDirective>,
        /// The tag directives list.
        tag_directives: Vec<TagDirective>,
        /// Is the document indicator implicit?
        implicit: bool,
    },
    /// The document end parameters (for `YAML_DOCUMENT_END_EVENT`).
    DocumentEnd {
        implicit: bool,
    },
    /// The alias parameters (for `YAML_ALIAS_EVENT`).
    Alias {
        /// The anchor.
        anchor: String,
    },
    /// The scalar parameters (for `YAML_SCALAR_EVENT`).
    Scalar {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// The scalar value.
        value: String,
        /// Is the tag optional for the plain style?
        plain_implicit: bool,
        /// Is the tag optional for any non-plain style?
        quoted_implicit: bool,
        /// The scalar style.
        style: ScalarStyle,
    },
    /// The sequence parameters (for `YAML_SEQUENCE_START_EVENT`).
    SequenceStart {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// Is the tag optional?
        implicit: bool,
        /// The sequence style.
        style: SequenceStyle,
    },
    SequenceEnd,
    /// The mapping parameters (for `YAML_MAPPING_START_EVENT`).
    MappingStart {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// Is the tag optional?
        implicit: bool,
        /// The mapping style.
        style: MappingStyle,
    },
    MappingEnd,
}

impl Event {
    /// Make an event from its data, setting both marks to zero.
    pub(crate) fn new(data: EventData) -> Self {
        Self {
            data,
            start_mark: Mark::default(),
            end_mark: Mark::default(),
        }
    }

    /// Create the STREAM-START event.
    pub fn stream_start(encoding: Encoding) -> Self {
        Self::new(EventData::StreamStart { encoding })
    }

    /// Create the STREAM-END event.
    pub fn stream_end() -> Self {
        Self::new(EventData::StreamEnd)
    }

    /// Create the DOCUMENT-START event.
    ///
    /// The `implicit` argument is considered as a stylistic parameter and may be
    /// ignored by the emitter.
    pub fn document_start(
        version_directive: Option<VersionDirective>,
        tag_directives_in: &[TagDirective],
        implicit: bool,
    ) -> Self {
        let tag_directives = tag_directives_in.to_vec();

        Self::new(EventData::DocumentStart {
            version_directive,
            tag_directives,
            implicit,
        })
    }

    /// Create the DOCUMENT-END event.
    ///
    /// The `implicit` argument is considered as a stylistic parameter and may be
    /// ignored by the emitter.
    pub fn document_end(implicit: bool) -> Self {
        Self::new(EventData::DocumentEnd { implicit })
    }

    /// Create an ALIAS event.
    pub fn alias(anchor: &str) -> Self {
        Self::new(EventData::Alias {
            anchor: String::from(anchor),
        })
    }

    /// Create a SCALAR event.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Either the `tag` attribute or one of the `plain_implicit` and
    /// `quoted_implicit` flags must be set.
    ///
    pub fn scalar(
        anchor: Option<&str>,
        tag: Option<&str>,
        value: impl Into<String>,
        plain_implicit: bool,
        quoted_implicit: bool,
        style: ScalarStyle,
    ) -> Self {
        let mut anchor_copy: Option<String> = None;
        let mut tag_copy: Option<String> = None;

        if let Some(anchor) = anchor {
            anchor_copy = Some(String::from(anchor));
        }
        if let Some(tag) = tag {
            tag_copy = Some(String::from(tag));
        }

        Self::new(EventData::Scalar {
            anchor: anchor_copy,
            tag: tag_copy,
            value: value.into(),
            plain_implicit,
            quoted_implicit,
            style,
        })
    }

    /// Create a SEQUENCE-START event.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Either the `tag` attribute or the `implicit` flag must be set.
    pub fn sequence_start(
        anchor: Option<&str>,
        tag: Option<&str>,
        implicit: bool,
        style: SequenceStyle,
    ) -> Self {
        let mut anchor_copy: Option<String> = None;
        let mut tag_copy: Option<String> = None;

        if let Some(anchor) = anchor {
            anchor_copy = Some(String::from(anchor));
        }
        if let Some(tag) = tag {
            tag_copy = Some(String::from(tag));
        }

        Self::new(EventData::SequenceStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        })
    }

    /// Create a SEQUENCE-END event.
    pub fn sequence_end() -> Self {
        Self::new(EventData::SequenceEnd)
    }

    /// Create a MAPPING-START event.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Either the `tag` attribute or the `implicit` flag must be set.
    pub fn mapping_start(
        anchor: Option<&str>,
        tag: Option<&str>,
        implicit: bool,
        style: MappingStyle,
    ) -> Self {
        let mut anchor_copy: Option<String> = None;
        let mut tag_copy: Option<String> = None;

        if let Some(anchor) = anchor {
            anchor_copy = Some(String::from(anchor));
        }

        if let Some(tag) = tag {
            tag_copy = Some(String::from(tag));
        }

        Self::new(EventData::MappingStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        })
    }

    /// Create a MAPPING-END event.
    pub fn mapping_end() -> Self {
        Self::new(EventData::MappingEnd)
    }
}

use serde::Serialize;

#[derive(Debug)]
pub enum AttachMetaError {
    InputError(String),
    ManifestError(String),
    TransportError(String),
    ProtocolError { response_json: serde_json::Value },
    InternalError(String),
}

impl std::fmt::Display for AttachMetaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputError(msg) => write!(f, "input error: {msg}"),
            Self::ManifestError(msg) => write!(f, "manifest error: {msg}"),
            Self::TransportError(msg) => write!(f, "transport error: {msg}"),
            Self::ProtocolError { .. } => write!(f, "protocol error: tool returned ok:false"),
            Self::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for AttachMetaError {}

impl AttachMetaError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ProtocolError { .. } => 1,
            _ => 2,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: String,
    pub category: &'static str,
}

impl ErrorEnvelope {
    pub fn from_error(err: &AttachMetaError) -> Self {
        let category = match err {
            AttachMetaError::InputError(_) => "input",
            AttachMetaError::ManifestError(_) => "manifest",
            AttachMetaError::TransportError(_) => "transport",
            AttachMetaError::ProtocolError { .. } => "protocol",
            AttachMetaError::InternalError(_) => "internal",
        };
        ErrorEnvelope {
            ok: false,
            error: err.to_string(),
            category,
        }
    }
}

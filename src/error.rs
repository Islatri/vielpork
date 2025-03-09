pub type Result<T> = core::result::Result<T, Error>;

pub struct Error {
    inner: Box<ErrorKind>
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            inner: Box::new(kind)
        }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for Error {}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}


impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::new(ErrorKind::StdIoError(e))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::new(ErrorKind::SerdeJsonError(e))
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::new(ErrorKind::ReqwestError(e))
    }
}

impl From<handlebars::RenderError> for Error {
    fn from(e: handlebars::RenderError) -> Self {
        Error::new(ErrorKind::HandlebarsRenderError(e))
    }
}
impl From<chrono::ParseError> for Error {
    fn from(e: chrono::ParseError) -> Self {
        Error::new(ErrorKind::ChronoParseError(e))
    }
}

#[cfg(feature = "tui")]
impl From<indicatif::style::TemplateError> for Error {
    fn from(e: indicatif::style::TemplateError) -> Self {
        Error::new(ErrorKind::IndicatifTemplateError(e))
    }
}
impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::new(ErrorKind::VielporkError(e))
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::new(ErrorKind::VielporkError(e.to_string()))
    }
}

pub enum ErrorKind {
    VielporkError(String),
    ReqwestError(reqwest::Error),
    StdIoError(std::io::Error),
    SerdeJsonError(serde_json::Error),
    HandlebarsRenderError(handlebars::RenderError),
    ChronoParseError(chrono::ParseError),
    #[cfg(feature = "tui")]
    IndicatifTemplateError(indicatif::style::TemplateError),
}

impl std::fmt::Debug for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::VielporkError(e) => write!(f, "{}", e),
            ErrorKind::ReqwestError(e) => write!(f, "{}", e),
            ErrorKind::StdIoError(e) => write!(f, "{}", e),
            ErrorKind::SerdeJsonError(e) => write!(f, "{}", e),
            ErrorKind::HandlebarsRenderError(e) => write!(f, "{}", e),
            ErrorKind::ChronoParseError(e) => write!(f, "{}", e),
            #[cfg(feature = "tui")]
            ErrorKind::IndicatifTemplateError(e) => write!(f, "{}", e),
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::VielporkError(e) => write!(f, "{}", e),
            ErrorKind::ReqwestError(e) => write!(f, "{}", e),
            ErrorKind::StdIoError(e) => write!(f, "{}", e),
            ErrorKind::SerdeJsonError(e) => write!(f, "{}", e),
            ErrorKind::HandlebarsRenderError(e) => write!(f, "{}", e),
            ErrorKind::ChronoParseError(e) => write!(f, "{}", e),
            #[cfg(feature = "tui")]
            ErrorKind::IndicatifTemplateError(e) => write!(f, "{}", e),
        }
    }
}
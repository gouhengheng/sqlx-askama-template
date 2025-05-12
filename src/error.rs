use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("data base error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("askama rerror: {0}")]
    AskamaError(#[from] askama::Error),
    #[error("AskamaTemplate error: {0}")]
    Message(String),
    #[error("fmt error: {0}")]
    FmtError(#[from] std::fmt::Error),
    #[error("AskamaTemplate MultipleErrors: {0:?}")]
    MultipleErrors(Vec<Error>),
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Message(e)
    }
}

impl From<&String> for Error {
    fn from(e: &String) -> Self {
        Error::Message(e.clone())
    }
}
impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::Message(e.to_string())
    }
}

impl From<Vec<Error>> for Error {
    fn from(e: Vec<Error>) -> Self {
        Error::MultipleErrors(e)
    }
}

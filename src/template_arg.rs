use sqlx::{Arguments, Database};

use crate::Error;
use std::cell::RefCell;
/// SQL template argument processor
///
/// Handles parameter encoding and binding for SQL templates
pub struct TemplateArg<'q, DB: Database> {
    /// Stores any encoding errors
    error: RefCell<Option<Error>>,
    /// Stores SQL parameters
    arguments: RefCell<Option<DB::Arguments<'q>>>,
    encode_placeholder_fn: Option<fn(usize, &mut String)>,
}

impl<DB: Database> Default for TemplateArg<'_, DB> {
    /// Creates default TemplateArg
    fn default() -> Self {
        TemplateArg {
            error: RefCell::new(None),
            arguments: RefCell::new(None),
            encode_placeholder_fn: None,
        }
    }
}

impl<'q, DB: Database> TemplateArg<'q, DB> {
    pub fn set_encode_placeholder_fn(&mut self, f: fn(usize, &mut String)) {
        self.encode_placeholder_fn = Some(f);
    }
    pub fn add_err(&self, e: impl Into<Error>) {
        let current_err = self.error.borrow_mut().take();
        if let Some(err) = current_err {
            if let Error::MultipleErrors(mut error) = err {
                error.push(e.into());
                self.error.replace(Some(Error::MultipleErrors(error)));
            } else {
                self.error
                    .replace(Some(Error::MultipleErrors(vec![err, e.into()])));
            }
        } else {
            self.error.replace(Some(e.into()));
        }
    }
    /// Encodes a single parameter and returns its placeholder
    ///
    /// # Arguments
    /// * `t` - Value to encode
    ///
    /// # Returns
    /// Parameter placeholder string (e.g. "$1" or "?")
    pub fn encode<T>(&self, t: T) -> String
    where
        T: sqlx::Encode<'q, DB> + sqlx::Type<DB> + 'q,
    {
        let mut arguments = self.arguments.borrow_mut().take().unwrap_or_default();

        if let Err(encode_err) = arguments.add(t) {
            self.add_err(sqlx::Error::Encode(encode_err));
        }

        let mut placeholder = String::new();
        if let Some(encode_placeholder_fn) = &self.encode_placeholder_fn {
            encode_placeholder_fn(arguments.len(), &mut placeholder);
        } else if let Err(e) = arguments.format_placeholder(&mut placeholder) {
            self.add_err(e);
        }
        *self.arguments.borrow_mut() = Some(arguments);
        placeholder
    }

    /// Encodes a parameter list and returns placeholder sequence
    ///
    /// # Arguments
    /// * `args` - Iterator of values to encode
    ///
    /// # Returns
    /// Parameter placeholder sequence (e.g. "($1,$2,$3)")
    pub fn encode_list<T>(&self, args: impl Iterator<Item = T>) -> String
    where
        T: sqlx::Encode<'q, DB> + sqlx::Type<DB> + 'q,
    {
        let mut placeholder = String::new();
        placeholder.push('(');

        for arg in args {
            placeholder.push_str(&self.encode(arg));

            placeholder.push(',');
        }

        if placeholder.ends_with(",") {
            placeholder.pop();
        }
        placeholder.push(')');

        placeholder
    }

    /// Takes any encoding error that occurred
    pub fn get_err(&self) -> Option<Error> {
        self.error.borrow_mut().take()
    }

    /// Takes ownership of the encoded arguments
    pub fn get_arguments(&self) -> Option<DB::Arguments<'q>> {
        self.arguments.borrow_mut().take()
    }
}

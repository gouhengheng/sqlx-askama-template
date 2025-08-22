use std::{cell::RefCell, ops::Deref};

use sqlx_core::{Error, arguments::Arguments, database::Database, encode::Encode, types::Type};
/// SQL template argument processor handling safe parameter encoding and placeholder generation
///
/// # Generic Parameters
/// - `'q`: Lifetime for database arguments
/// - `DB`: Database type implementing [`sqlx::Database`]
/// - `D`: Template data type
pub struct TemplateArg<'q, DB: Database, D> {
    /// Stores any encoding errors
    error: RefCell<Option<Error>>,
    /// Stores SQL parameters
    arguments: RefCell<Option<DB::Arguments<'q>>>,
    encode_placeholder_fn: Option<fn(usize, &mut String)>,
    data: &'q D,
}

impl<'q, DB: Database, D> TemplateArg<'q, DB, D> {
    /// Creates a new TemplateArg instance wrapping template data
    ///
    /// # Arguments
    /// * `d` - Reference to template data with lifetime `'q`
    pub fn new(d: &'q D) -> Self {
        TemplateArg {
            error: RefCell::new(None),
            arguments: RefCell::new(None),
            encode_placeholder_fn: None,
            data: d,
        }
    }
    /// Sets custom placeholder formatting function
    ///
    /// # Arguments
    /// * `f` - Function that takes parameter index and appends placeholder
    pub fn set_encode_placeholder_fn(&mut self, f: fn(usize, &mut String)) {
        self.encode_placeholder_fn = Some(f);
    }

    /// Encodes a single parameter and returns its placeholder
    ///
    /// # Arguments
    /// * `t` - Value implementing [`sqlx::Encode`] and [`sqlx::Type`]
    ///
    /// # Returns
    /// Placeholder string (e.g., `$1` or `?`)
    ///
    /// # Example
    /// ```
    /// let placeholder = arg.e(user_id);
    /// ```
    pub fn e<ImplEncode>(&self, t: ImplEncode) -> String
    where
        ImplEncode: Encode<'q, DB> + Type<DB> + 'q,
    {
        let mut arguments = self.arguments.borrow_mut().take().unwrap_or_default();
        let mut err = self.error.borrow_mut();

        if let Err(encode_err) = arguments.add(t)
            && err.is_none()
        {
            *err = Some(Error::Encode(encode_err));
        }

        let mut placeholder = String::new();
        if let Some(encode_placeholder_fn) = &self.encode_placeholder_fn {
            encode_placeholder_fn(arguments.len(), &mut placeholder);
        } else if let Err(e) = arguments.format_placeholder(&mut placeholder) {
            *err = Some(Error::Encode(Box::new(e)));
        }
        *self.arguments.borrow_mut() = Some(arguments);
        placeholder
    }
    /// Encodes an iterable of parameters and returns parenthesized placeholders
    ///
    /// # Arguments
    /// * `args` - Iterator of encodable values
    ///
    /// # Returns
    /// Comma-separated placeholders wrapped in parentheses
    ///
    /// # Example
    /// ```
    /// let placeholders = arg.el(&[1, 2, 3]);
    /// ```
    pub fn el<ImplEncode>(&self, args: impl ::std::iter::IntoIterator<Item = ImplEncode>) -> String
    where
        ImplEncode: Encode<'q, DB> + Type<DB> + 'q,
    {
        let mut placeholder = String::new();
        placeholder.push('(');

        for arg in args {
            placeholder.push_str(&self.e(arg));

            placeholder.push(',');
        }

        if placeholder.ends_with(",") {
            placeholder.pop();
        }
        placeholder.push(')');

        placeholder
    }
    /// Clone-and-encode variant for types requiring cloning
    ///
    /// # Arguments
    /// * `t` - Reference to clonable encodable value
    pub fn et<ImplEncode>(&self, t: &ImplEncode) -> ::std::string::String
    where
        ImplEncode: Encode<'q, DB> + Type<DB> + ::std::clone::Clone + 'q,
    {
        self.e(t.clone())
    }
    /// Clone-and-encode list variant
    ///
    /// # Arguments
    /// * `args` - Iterator of references to clonable values
    pub fn etl<'arg_b, ImplEncode>(
        &self,
        args: impl ::std::iter::IntoIterator<Item = &'arg_b ImplEncode>,
    ) -> ::std::string::String
    where
        'q: 'arg_b,
        ImplEncode: Encode<'q, DB> + Type<DB> + ::std::clone::Clone + 'q,
    {
        let args = args.into_iter().cloned();
        self.el(args)
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

impl<'q, DB: Database, D> Deref for TemplateArg<'q, DB, D> {
    type Target = &'q D;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

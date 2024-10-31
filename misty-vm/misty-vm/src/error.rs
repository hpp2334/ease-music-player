pub trait IntoVMError {
    fn cast(self) -> Box<dyn std::error::Error>;
}

impl<E> IntoVMError for E
where
    E: Into<Box<dyn std::error::Error>> + 'static,
{
    fn cast(self) -> Box<dyn std::error::Error> {
        self.into()
    }
}

// TODO: move to some other file?
/// This is utility trait which allows using `result.ignore()` rather than `let _ = result`.
pub trait Ignore {
    fn ignore(self);
}

impl<T, E> Ignore for Result<T, E> {
    #[inline(always)]
    fn ignore(self) {}
}

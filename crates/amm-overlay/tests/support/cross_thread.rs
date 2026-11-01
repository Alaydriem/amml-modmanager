/// Moves a value to another thread in a test that knows only one thread uses it at a time.
pub struct CrossThread<T>(pub T);

// SAFETY: each test that wraps a value hands it to one thread and does not touch it meanwhile
unsafe impl<T> Send for CrossThread<T> {}

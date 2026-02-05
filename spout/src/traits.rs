/// Consumes items.
pub trait Spout<T> {
    /// Consume an item.
    fn send(&mut self, item: T);

    /// Consume multiple items from an iterator.
    ///
    /// Default implementation calls `send` for each item.
    /// Implementors can override for batch optimizations.
    #[inline]
    fn send_all(&mut self, items: impl Iterator<Item = T>) {
        for item in items {
            self.send(item);
        }
    }

    /// Flush buffered data.
    #[inline]
    fn flush(&mut self) {}
}

/// Flush behavior.
pub trait Flush {
    /// Perform flush.
    fn flush(&mut self);
}

impl Flush for () {
    #[inline]
    fn flush(&mut self) {}
}

impl<F: FnMut()> Flush for F {
    #[inline]
    fn flush(&mut self) {
        self()
    }
}

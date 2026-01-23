//! Ring buffer traits.

/// Ring buffer producer.
pub trait RingProducer<T> {
    /// Try to push. Returns `Err(item)` if full.
    fn try_push(&mut self, item: T) -> Result<(), T>;

    /// True if full.
    fn is_full(&self) -> bool;

    /// Capacity.
    fn capacity(&self) -> usize;

    /// Current length.
    fn len(&self) -> usize;

    /// True if empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Ring buffer consumer.
pub trait RingConsumer<T> {
    /// Try to pop. Returns `None` if empty.
    #[must_use]
    fn try_pop(&mut self) -> Option<T>;

    /// Peek at oldest item.
    #[must_use]
    fn peek(&self) -> Option<&T>;

    /// True if empty.
    fn is_empty(&self) -> bool;

    /// Current length.
    fn len(&self) -> usize;

    /// Capacity.
    fn capacity(&self) -> usize;
}

/// Combined producer and consumer.
pub trait RingTrait<T>: RingProducer<T> + RingConsumer<T> {}

impl<T, R: RingProducer<T> + RingConsumer<T>> RingTrait<T> for R {}

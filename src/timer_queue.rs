use std::cmp::Ordering;
use std::time::Instant;
use std::collections::BinaryHeap;



struct Item<T:Sized> {
    pub deadline: Instant,
    value: T,
}

impl<T> PartialEq for Item<T> {
    fn eq(&self, other: &Item<T>) -> bool {
        return other.deadline.eq(&self.deadline);
    }
}

impl<T> PartialOrd for Item<T> {
    fn partial_cmp(&self, other: &Item<T>) -> Option<Ordering> {
        // Turning max-heap upside down
        return other.deadline.partial_cmp(&self.deadline);
    }
}

impl<T> Eq for Item<T> {}
impl<T> Ord for Item<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        return other.deadline.cmp(&self.deadline);
    }
}

pub struct Queue<T:Sized>(BinaryHeap<Item<T>>);

pub struct QueueIter<'a, T> where T: 'a {
    queue: &'a mut Queue<T>,
    max_time: Instant,
}

impl<'a, T> Iterator for QueueIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.queue.peek_time().map(|x| x < self.max_time).unwrap_or(false) {
            self.queue.0.pop().map(|x| x.value)
        } else {
            None
        }
    }
}

impl<T> Queue<T> {
    pub fn new() -> Queue<T> {
        Queue(BinaryHeap::new())
    }
    pub fn add(&mut self, deadline: Instant, value: T) {
        self.0.push(Item { deadline: deadline, value: value });
    }
    pub fn peek_time(&self) -> Option<Instant> {
        return self.0.peek().map(|x| x.deadline)
    }
    pub fn pop_until<'x>(&'x mut self, max_time: Instant)
        -> QueueIter<'x, T>
    {
        QueueIter { queue: self, max_time: max_time }
    }
}

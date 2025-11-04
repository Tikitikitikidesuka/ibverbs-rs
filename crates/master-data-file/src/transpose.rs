pub struct Transpose<I: Iterator> {
    iters: Vec<I>,
    next: usize,
}

impl<T, I: Iterator<Item = T>> Iterator for Transpose<I> {
    type Item = Box<dyn Iterator<Item = T>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut iter = self
            .iters
            .iter_mut()
            .filter_map(|iter| iter.next())
            .peekable();
        if iter.peek().is_some() {
            Some(Box::new(iter))
        } else {
            None
        }
    }
}

pub trait Iterator2D<J: IntoIterator>: Iterator<Item = J> + Sized {
    fn transpose(self) -> Transpose<<J as IntoIterator>::IntoIter>;
}

impl<I: Iterator<Item = J>, J: IntoIterator> Iterator2D<J> for I {
    fn transpose(self) -> Transpose<<J as IntoIterator>::IntoIter> {
        Transpose {
            iters: self.map(|i| i.into_iter()).collect(),
            next: 0,
        }
    }
}

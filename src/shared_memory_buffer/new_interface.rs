pub trait ReadableElement {
    fn size(&self) -> usize;
}

pub trait WrappableReadableElement: ReadableElement {
    fn valid(&self) -> bool;
    fn wraps(&self) -> bool;
}

pub trait WrappableReadableBuffer {
    fn read<T: WrappableReadableElement>(&mut self) -> ReadGuard;
}

pub trait ReadableBuffer {
    fn read<T: WrappableReadableElement>(&mut self) -> ReadGuard;
}

pub struct ReadGuard<'a> {
    data: &'a [u8],
}

/*
pub struct BufferReader<B: Buffer> {
    buffer: B,
}

impl<B: Buffer> BufferReader<B> {
    pub fn new(buffer: B) -> Self {
        Self { buffer }
    }

    fn read<T: ReadableElement + BufferElement>(&mut self, ) -> ReadGuard<Self> {
        todo!()
    }

    fn read_multiple<T: ReadableElement + BufferElement>() -> MultiReadGuard<Self> {
        todo!()
    }
}

pub struct BufferWriter<B: Buffer> {
    buffer: B,
}

impl<B: Buffer> BufferWriter<B> {
    fn new(buffer: B) -> Self {
        Self { buffer }
    }

    fn write<T: WritableElement + BufferElement>(element: T) {
        todo!()
    }

    fn write_multiple<T: WritableElement + BufferElement>(elements: impl IntoIterator<Item = T>) {
        todo!()
    }
}

pub trait BufferElement {
    fn size() -> usize;
    fn wraps() -> bool;
}

pub trait ReadableElement {
    fn find(data: &[u8]) -> usize;
}

pub trait WritableElement {
    fn write(self, buffer: &mut [u8]) -> usize;
}

pub struct ReadGuard<'a, R: BufferReader> {
    reader: &'a R,
}

pub struct MultiReadGuard<'a, R: BufferReader> {
    readers: &'a [R],
}

impl<'a, R: BufferReader> ReadGuard<'a, R> {
    pub fn new(reader: &R) -> ReadGuard<'a, R> {
        Self { reader }
    }
}


 */
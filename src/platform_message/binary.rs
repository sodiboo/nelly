use std::{
    io::{Cursor, Read, Result, Seek, Write},
    mem::MaybeUninit,
};

pub trait BinaryEncodable {
    fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()>;
}

pub trait BinaryDecodable: Sized {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self>;
}

/// Write to a stream.
pub struct BinaryWriter<W: Write> {
    stream: W,
}

impl<W: Write> BinaryWriter<W> {
    pub fn new(stream: W) -> Self {
        Self { stream }
    }

    fn write_bytes(&mut self, bytes: impl AsRef<[u8]>) -> Result<()> {
        self.stream.write_all(bytes.as_ref())
    }

    pub fn write<T: BinaryEncodable>(&mut self, value: &T) -> Result<()> {
        value.encode(self)
    }

    pub fn write_slice<T: BinaryEncodable>(&mut self, slice: &impl AsRef<[T]>) -> Result<()> {
        for elem in slice.as_ref() {
            elem.encode(self)?;
        }
        Ok(())
    }

    pub fn write_string(&mut self, string: &impl AsRef<str>) -> Result<()> {
        let string = string.as_ref();
        let length = string.len() as u64;
        self.write(&length)?;
        self.write_bytes(string.as_bytes())
    }
}

pub struct BinaryReader<R: Read + Seek> {
    stream: R,
}

impl<R: Read + Seek> BinaryReader<R> {
    pub fn new(stream: R) -> Self {
        Self { stream }
    }

    pub fn assert_finished(&mut self) -> Result<()> {
        let current = self.stream.stream_position()?;
        let end = self.stream.seek(std::io::SeekFrom::End(0))?;

        if current == end {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "additional data at end of stream",
            ))
        }
    }

    fn fill_bytes<T: AsMut<[u8]>>(&mut self, mut slice: T) -> Result<T> {
        self.stream.read_exact(slice.as_mut()).map(|()| slice)
    }

    pub fn read<T: BinaryDecodable>(&mut self) -> Result<T> {
        T::decode(self)
    }

    pub fn read_string(&mut self) -> Result<String> {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Dart doesn't really let me encode usize, so they're always widened to u64 \
                                (which is the same size, on all modern systems, anyway)"
        )]
        let len = self.read::<u64>()? as usize;
        String::from_utf8(self.fill_bytes(vec![0; len])?).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid utf-8 sequence in string",
            )
        })
    }

    pub fn read_vec<T: BinaryDecodable>(&mut self, len: usize) -> Result<Vec<T>> {
        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            vec.push(self.read()?);
        }
        Ok(vec)
    }

    pub fn read_array<T: BinaryDecodable, const N: usize>(&mut self) -> Result<[T; N]> {
        let mut array = [const { MaybeUninit::<T>::uninit() }; N];
        for elem in &mut array {
            elem.write(self.read()?);
        }
        Ok(unsafe { std::mem::transmute_copy(&array) })
    }
}

impl<'a, T: ?Sized + AsRef<[u8]>> From<&'a T> for BinaryReader<Cursor<&'a [u8]>> {
    fn from(data: &'a T) -> Self {
        Self {
            stream: Cursor::new(data.as_ref()),
        }
    }
}

macro_rules! impl_scalar_encodable {
    ($($ty:ty),* $(,)?) => {
        $(
            impl BinaryEncodable for $ty {
                fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
                    writer.write_bytes(self.to_ne_bytes())
                }
            }

            impl BinaryDecodable for $ty {
                fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
                    reader.fill_bytes([0; std::mem::size_of::<Self>()]).map(Self::from_ne_bytes)
                }
            }
        )*
    };
}

impl_scalar_encodable! {
    u8, u16, u32, u64, // u128, usize,
    i8, i16, i32, i64, // i128, isize,
    f32, f64,
}

// i64 because it matches the Dart VM's `int` type
impl BinaryEncodable for volito::ViewId {
    fn encode(&self, writer: &mut BinaryWriter<impl Write>) -> Result<()> {
        writer.write::<i64>(&self.0)
    }
}
impl BinaryDecodable for volito::ViewId {
    fn decode(reader: &mut BinaryReader<impl Read + Seek>) -> Result<Self> {
        Ok(volito::ViewId(reader.read::<i64>()?))
    }
}

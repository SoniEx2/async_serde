//! serde to async-serde bridge

use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::task::noop_waker;

use serde::de::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::EnumAccess;
use serde::de::Error as _;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;

use super::Data;
use super::Deserialize;
use super::InternalOp;
use super::State;
use super::Visit;

/// The bridge itself.
pub(in super) struct Bridge<'s, 'de, F: ?Sized> {
    pub(in super) state: &'s State<'de>,
    pub(in super) fut: Pin<Box<F>>,
}

/// Polls the given future with a no-op waker.
fn quick_poll<F: Future + ?Sized>(fut: Pin<&mut F>) -> Poll<F::Output> {
    // there is no waker
    let waker = noop_waker();
    let mut ctx = Context::from_waker(&waker);
    fut.poll(&mut ctx)
}

impl<'s, 'de, F, T, E> DeserializeSeed<'de> for Bridge<'s, 'de, F>
where
    F: Future<Output=Result<T, E>> + ?Sized
{
    type Value = Poll<F::Output>;
    fn deserialize<D>(mut self, d: D) -> Result<Poll<F::Output>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match quick_poll(self.fut.as_mut()) {
            Poll::Pending => {
                match self.state.internal_op.take() {
                    None => {},
                    _ => unreachable!(),
                }
                let depth;
                depth = self.state.depth.replace(self.state.depth.get() + 1);
                if depth != self.state.expected_depth.get() {
                    panic!("not sure what causes this to happen");
                }
                match self.state.deserialize.take() {
                    Some(Deserialize::Any) => {
                        d.deserialize_any(self)
                    },
                    Some(Deserialize::IgnoredAny) => {
                        d.deserialize_ignored_any(self)
                    },
                    Some(Deserialize::Bool) => {
                        d.deserialize_bool(self)
                    },
                    Some(Deserialize::I8) => {
                        d.deserialize_i8(self)
                    },
                    Some(Deserialize::I16) => {
                        d.deserialize_i16(self)
                    },
                    Some(Deserialize::I32) => {
                        d.deserialize_i32(self)
                    },
                    Some(Deserialize::I64) => {
                        d.deserialize_i64(self)
                    },
                    Some(Deserialize::I128) => {
                        d.deserialize_i128(self)
                    },
                    Some(Deserialize::U8) => {
                        d.deserialize_u8(self)
                    },
                    Some(Deserialize::U16) => {
                        d.deserialize_u16(self)
                    },
                    Some(Deserialize::U32) => {
                        d.deserialize_u32(self)
                    },
                    Some(Deserialize::U64) => {
                        d.deserialize_u64(self)
                    },
                    Some(Deserialize::U128) => {
                        d.deserialize_u128(self)
                    },
                    Some(Deserialize::F32) => {
                        d.deserialize_f32(self)
                    },
                    Some(Deserialize::F64) => {
                        d.deserialize_f64(self)
                    },
                    Some(Deserialize::Char) => {
                        d.deserialize_char(self)
                    },
                    Some(Deserialize::Option) => {
                        d.deserialize_option(self)
                    },
                    Some(Deserialize::Unit) => {
                        d.deserialize_unit(self)
                    },
                    Some(Deserialize::Seq) => {
                        d.deserialize_seq(self)
                    },
                    Some(Deserialize::Map) => {
                        d.deserialize_map(self)
                    },
                    Some(Deserialize::Identifier) => {
                        d.deserialize_identifier(self)
                    },
                    Some(Deserialize::Bytes) => {
                        d.deserialize_bytes(self)
                    },
                    Some(Deserialize::ByteBuf) => {
                        d.deserialize_byte_buf(self)
                    },
                    Some(Deserialize::Str) => {
                        d.deserialize_str(self)
                    },
                    Some(Deserialize::String) => {
                        d.deserialize_string(self)
                    },
                    Some(Deserialize::Tuple(size)) => {
                        d.deserialize_tuple(size, self)
                    },
                    Some(Deserialize::UnitStruct { name }) => {
                        d.deserialize_unit_struct(name, self)
                    },
                    Some(Deserialize::TupleStruct { name, len }) => {
                        d.deserialize_tuple_struct(name, len, self)
                    },
                    Some(Deserialize::NewtypeStruct { name }) => {
                        d.deserialize_newtype_struct(name, self)
                    },
                    Some(Deserialize::Struct { name, fields }) => {
                        d.deserialize_struct(name, fields, self)
                    },
                    Some(Deserialize::Enum { name, variants }) => {
                        d.deserialize_enum(name, variants, self)
                    },
                    None => {
                        Err(D::Error::custom("awaiting on incorrect future"))
                    },
                }
            },
            result => {
                Ok(result)
            },
        }
    }
}

impl<'s, 'de, F, T, U> Visitor<'de> for Bridge<'s, 'de, F>
where
    F: Future<Output=Result<T, U>> + ?Sized
{
    type Value = Poll<F::Output>;

    fn expecting(&self, _: &mut std::fmt::Formatter) -> std::fmt::Result {
        unreachable!()
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_str<E>(mut self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        struct BufferedStr<'a, 'de>(&'a State<'de>);
        impl<'a, 'de> Drop for BufferedStr<'a, 'de> {
            fn drop(&mut self) {
                let BufferedStr(state) = self;
                // SAFETY: cannot be borrwed by Future.
                // we must clean this up here otherwise we get/risk UB.
                unsafe {
                    *state.data.get() = Data::None;
                }
            }
        }
        let _guard = BufferedStr(self.state);
        // SAFETY: cannot be borrowed by Future.
        // using drop guard.
        unsafe {
            *self.state.data.get() = Data::Str(v.into())
        }
        self.state.visit.set(Some(Visit::Str));
        match quick_poll(self.fut.as_mut()) {
            Poll::Pending => {
                todo!()
            },
            result => {
                Ok(result)
            },
        }
    }
    fn visit_borrowed_str<E>(mut self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // SAFETY: cannot be borrowed by Future.
        // we don't bother with cleanup because this is borrowed for 'de.
        unsafe {
            *self.state.data.get() = Data::DeStr(v);
        }
        self.state.visit.set(Some(Visit::Str));
        match quick_poll(self.fut.as_mut()) {
            Poll::Pending => {
                todo!()
            },
            result => {
                Ok(result)
            },
        }
    }
    fn visit_string<E>(mut self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // SAFETY: cannot be borrowed by Future.
        // we don't bother with cleanup because this is owned.
        unsafe {
            *self.state.data.get() = Data::String(v.into());
        }
        self.state.visit.set(Some(Visit::Str));
        match quick_poll(self.fut.as_mut()) {
            Poll::Pending => {
                todo!()
            },
            result => {
                Ok(result)
            },
        }
    }
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        todo!()
    }
    fn visit_newtype_struct<D>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
    fn visit_seq<A>(mut self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.state.visit.set(Some(Visit::Seq));
        match quick_poll(self.fut.as_mut()) {
            Poll::Pending => {
                todo!()
            },
            result => {
                Ok(result)
            },
        }
    }
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        todo!()
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        todo!()
    }
}

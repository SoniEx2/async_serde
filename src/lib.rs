//! # Async Serde
//!
//! This is a crate for `async fn` serde. It's not necessarily more efficient,
//! merely different. It allows using serde to interact with data formats (like
//! JSON, etc) directly instead of going through deserialization.
//!
//! Its main feature is the ability to trivially apply non-deterministic (in
//! the automaton sense) transformations directly onto the deserialization
//! process. This is particularly useful if you happen to be making a jq clone
//! that operates directly on the data stream instead of loading the whole data
//! stream into an in-memory data structure first.
//!
//! # Note
//!
//! Note that the types `Instructor` and `Step` can be passed around freely in
//! a `Run` impl. While not unsound, this can lead to hard-to-debug errors.
//! It's recommended to only use these as indicated in these examples, to avoid
//! the hard-to-debug runtime errors.
//!
//! # Examples
//!
//! ```rust
//! use std::borrow::Cow;
//!
//! use async_serde::Deserialize;
//! use async_serde::Inspect;
//! use async_serde::Instructor;
//! use async_serde::Run;
//! use async_serde::Visit;
//! use async_trait::async_trait;
//!
//! struct Foo;
//! #[async_trait(?Send)]
//! impl<'de, E: serde::de::Error> Run<'de, E> for Foo {
//!     type Output = Cow<'de, str>;
//!     async fn run(
//!         self,
//!         instructor: Instructor<'_, 'de>
//!     ) -> Result<Self::Output, E> {
//!         let step = instructor.ask(Deserialize::String).unwrap().await;
//!         match step.kind() {
//!             Visit::Str => {
//!                 step.inspect_string(|s| {
//!                     match s {
//!                         Inspect::Borrowed(s) => Cow::from(s),
//!                         Inspect::Owned(s) => Cow::from(String::from(s)),
//!                         Inspect::Buffered(s) => Cow::from(s.to_owned()),
//!                     }
//!                 }).ok_or_else(|| E::custom("wrong type"))
//!             },
//!             _ => Err(E::custom("wrong type")),
//!         }
//!     }
//! }
//!
//! fn main() {
//!     let mut json = serde_json::Deserializer::from_str("\"hello\"");
//!     let exec = async_serde::Executor::new(&mut json);
//!     // this is returning an Result<Option<Cow<'de, str>>, Error>
//!     let res = exec.run::<_, serde_json::Error>(Foo);
//!     assert_eq!(res.unwrap(), "hello");
//! }
//! ```
//!
//! ```rust
//! use std::borrow::Cow;
//!
//! use async_serde::Deserialize;
//! use async_serde::Inspect;
//! use async_serde::Instructor;
//! use async_serde::Run;
//! use async_serde::Visit;
//! use async_trait::async_trait;
//!
//! struct Foo;
//! #[async_trait(?Send)]
//! impl<'de, E: serde::de::Error> Run<'de, E> for Foo {
//!     type Output = [Vec<Cow<'de, str>>; 2];
//!     async fn run(
//!         self,
//!         instructor: Instructor<'_, 'de>
//!     ) -> Result<Self::Output, E> {
//!         let mut step = instructor.ask(Deserialize::Seq).unwrap().await;
//!         match step.kind() {
//!             Visit::Seq => {
//!                 let step2 = step.cloned();
//!                 match futures::join!(
//!                     step.inspect_seq(|s| async move {
//!                         // TODO
//!                         Ok(Vec::new())
//!                     }).ok_or_else(|| E::custom("wrong type"))?,
//!                     step2.inspect_seq(|s| async move {
//!                         // TODO
//!                         Ok(Vec::new())
//!                     }).ok_or_else(|| E::custom("wrong type"))?
//!                 ) {
//!                     (a, b) => Ok([a?, b?])
//!                 }
//!             },
//!             _ => Err(E::custom("wrong type")),
//!         }
//!     }
//! }
//!
//! fn main() {
//!     let mut json = serde_json::Deserializer::from_str("[\"a\", \"b\"]");
//!     let exec = async_serde::Executor::new(&mut json);
//!     // this is returning an Result<Option<Cow<'de, str>>, Error>
//!     let res = exec.run::<_, serde_json::Error>(Foo).unwrap();
//!     assert_eq!(res[0], &[Cow::from("a"), "b".into()][..]);
//!     assert_eq!(res[1], &[Cow::from("a"), "b".into()][..]);
//! }
//! ```

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::future::Future;
use std::mem;
use std::ptr::NonNull;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;

mod de;

#[async_trait(?Send)]
pub trait Run<'de, E> {
    type Output: 'de;

    async fn run<'s>(
        self,
        instructor: Instructor<'s, 'de>,
    ) -> Result<Self::Output, E>;
}

/// The Async Serde executor.
pub struct Executor<D> {
    deserializer: D,
}

impl<'de, D: serde::Deserializer<'de>> Executor<D> {
    /// Creates the executor.
    pub fn new(deserializer: D) -> Self {
        Self {
            deserializer,
        }
    }

    /// The main entry point to the executor.
    pub fn run<R: Run<'de, E>, E>(self, r: R) -> Result<R::Output, E>
    where
        E: From<D::Error>,
    {
        let deserializer = self.deserializer;
        let state = State {
            internal_op: Cell::new(None),
            data: UnsafeCell::new(Data::None),
            depth: Cell::new(0),
            expected_depth: Cell::new(0),
            deserialize: Cell::new(None),
            visit: Cell::new(None),
        };
        let fut = r.run(Instructor {
            state: &state,
            cloned: false,
            depth: 0,
        });
        match <de::Bridge<'_, '_, _> as serde::de::DeserializeSeed<'_>>::deserialize(
            de::Bridge {
                state: &state,
                fut: fut,
            },
            deserializer,
        )? {
            Poll::Pending => unreachable!(),
            Poll::Ready(result) => result,
        }
    }
}

/// Allows passing instructions to the Async Serde executor to iterate a
/// sequence.
pub struct SeqInstructor<'s, 'de> {
    /// The executor state.
    state: &'s State<'de>,
    /// Whether we need to clone/borrow Strings and ByteBufs.
    cloned: bool,
    depth: usize,
}

/// Allows passing instructions to the Async Serde executor.
#[derive(Debug)]
pub struct Instructor<'s, 'de> {
    /// The executor state.
    state: &'s State<'de>,
    /// Whether we need to clone/borrow Strings and ByteBufs.
    cloned: bool,
    depth: usize,
}

impl<'s, 'de> SeqInstructor<'s, 'de> {
    /// Marks this `SeqInstructor` as cloned and returns a clone.
    pub fn cloned(&mut self) -> Self {
        self.cloned = true;
        Self {
            state: self.state,
            cloned: self.cloned,
            depth: self.depth,
        }
    }

    /// Asks for a type from this instructor.
    ///
    /// # Panics
    ///
    /// Panics if called from the wrong depth (`Instructor` moved into
    /// incorrect future).
    pub fn ask(
        &mut self,
        deserialize: Deserialize,
    ) -> Result<impl Future<Output=Step<'s, 'de>> + '_, Option<Deserialize>> {
        if self.state.depth.get() != self.depth {
            panic!("wrong depth");
        }
        let internal_op = self.state.internal_op.get();
        match internal_op {
            None => {},
            _ => todo!()
        }
        let current = self.state.deserialize.get();
        // check if types are compatible
        match (current, deserialize) {
            (Some(current), visit) if current == visit => {},
            (Some(current), _) => {
                return Err(Some(current))
            },
            _ => {},
        }
        self.state.deserialize.set(Some(deserialize));
        Ok(futures::future::poll_fn(move |_cx: &mut Context<'_>| {
            self.state.expected_depth.set(self.depth);
            match self.state.visit.get() {
                None => Poll::Pending,
                Some(_) => {
                    Poll::Ready(Step {
                        state: self.state,
                        cloned: self.cloned,
                        depth: self.depth + 1,
                    })
                },
            }
        }))
    }
}

impl<'s, 'de> Instructor<'s, 'de> {
    /// Marks this `Instructor` as cloned and returns a clone.
    pub fn cloned(&mut self) -> Self {
        self.cloned = true;
        Self {
            state: self.state,
            cloned: self.cloned,
            depth: self.depth,
        }
    }

    /// Asks for a type from this instructor.
    ///
    /// # Panics
    ///
    /// Panics if called from the wrong depth (`Instructor` moved into
    /// incorrect future).
    pub fn ask(
        self,
        deserialize: Deserialize,
    ) -> Result<impl Future<Output=Step<'s, 'de>>, (Self, Deserialize)> {
        if self.state.depth.get() != self.depth {
            panic!("wrong depth");
        }
        let current = self.state.deserialize.get();
        // check if types are compatible
        match (current, deserialize) {
            (Some(current), visit) if current == visit => {},
            (Some(current), _) => {
                return Err((self, current))
            },
            _ => {},
        }
        self.state.deserialize.set(Some(deserialize));
        Ok(futures::future::poll_fn(move |_cx: &mut Context<'_>| {
            self.state.expected_depth.set(self.depth);
            match self.state.visit.get() {
                None => Poll::Pending,
                Some(_) => {
                    Poll::Ready(Step {
                        state: self.state,
                        cloned: self.cloned,
                        depth: self.depth + 1,
                    })
                },
            }
        }))
    }
}

/// Allows retrieving values from the Async Serde executor.
#[derive(Debug)]
pub struct Step<'s, 'de> {
    /// The executor state.
    state: &'s State<'de>,
    /// Whether we need to clone/borrow Strings and ByteBufs.
    cloned: bool,
    depth: usize,
}

impl<'s, 'de> Step<'s, 'de> {
    /// Returns the type/value the executor expects you to handle.
    ///
    /// # Panics
    ///
    /// Panics if called in an invalid state.
    pub fn kind(&self) -> Visit {
        if self.state.depth.get() != self.depth {
            panic!("Wrong depth");
        }
        self.state.visit.get().expect("Must not be called after call to ask")
    }

    /// Marks this `Step` as cloned and returns a clone.
    pub fn cloned(&mut self) -> Self {
        self.cloned = true;
        Self {
            state: self.state,
            cloned: self.cloned,
            depth: self.depth,
        }
    }

    /// Attempts to inspect a string.
    pub fn inspect_string<F, R>(self, f: F) -> Option<R>
    where
        F: for<'a> FnOnce(Inspect<'a, 'de, str>) -> R
    {
        if matches!(self.kind(), Visit::Str) {
            let data = self.state.data.get();
            // SAFETY: this is sync.
            match unsafe { &*data } {
                Data::DeStr(s) => {
                    return Some(f(Inspect::Borrowed(s)));
                },
                Data::String(ref s) if self.cloned => {
                    return Some(f(Inspect::Buffered(&**s)));
                },
                Data::Str(s) => {
                    // SAFETY: guaranteed to be valid by other parts of this
                    // code.
                    return Some(f(Inspect::Buffered(unsafe { s.as_ref() })));
                },
                Data::String(_) => {
                    // handled later
                },
                _ => unreachable!(),
            }
            // SAFETY: we're no longer borrowing `data` here, so this is okay.
            let s = mem::replace(unsafe { &mut *data }, Data::None);
            if let Data::String(s) = s {
                Some(f(Inspect::Owned(s)))
            } else {
                unreachable!()
            }
        } else {
            None
        }
    }

    /// Attempts to inspect bytes.
    pub fn inspect_bytes<F, R>(self, f: F) -> Option<R>
    where
        F: for<'a> FnOnce(Inspect<'a, 'de, [u8]>) -> R
    {
        if matches!(self.kind(), Visit::Bytes) {
            let data = self.state.data.get();
            // SAFETY: this is sync.
            match unsafe { &*data } {
                Data::DeBytes(s) => {
                    return Some(f(Inspect::Borrowed(s)));
                },
                Data::ByteBuf(ref s) if self.cloned => {
                    return Some(f(Inspect::Buffered(&**s)));
                },
                Data::Bytes(s) => {
                    // SAFETY: guaranteed to be valid by other parts of this
                    // code.
                    return Some(f(Inspect::Buffered(unsafe { s.as_ref() })));
                },
                Data::ByteBuf(_) => {
                    // handled later
                },
                _ => unreachable!(),
            }
            // SAFETY: we're no longer borrowing `data` here, so this is okay.
            let s = mem::replace(unsafe { &mut *data }, Data::None);
            if let Data::ByteBuf(s) = s {
                Some(f(Inspect::Owned(s)))
            } else {
                unreachable!()
            }
        } else {
            None
        }
    }

    /// Attempts to inspect a sequence.
    pub fn inspect_seq<Fun, Fut, R>(
        self,
        f: Fun,
    ) -> Option<impl Future<Output=R>>
    where
        Fun: FnOnce(SeqInstructor<'s, 'de>) -> Fut,
        Fut: Future<Output=R> + 's + 'de,
    {
        if matches!(self.kind(), Visit::Seq) {
            Some(async {
                todo!()
            })
        } else {
            None
        }
    }

    //pub fn inspect_some<Fun, Fut, R>(
    //    self,
    //    f: Fun,
    //) -> impl Future<Output=R>
    //where
    //    Fun: FnOnce(Instructor<'s, 'de>) -> Fut,
    //    Fut: Future<Output=R> + 's + 'de,
    //{
    //    async {
    //        todo!()
    //    }
    //}

    //pub fn inspect_newtype_struct<R: Run<'de>>(
    //    self,
    //    r: R,
    //) -> impl Future<Output=<R as Run<'de>>::Output> {
    //    async {
    //        todo!()
    //    }
    //}

    // TODO: Some, NewtypeStruct, Enum, Seq, Map
}

/// A deserialized buffer for inspection.
pub enum Inspect<'a, 'de, T: ?Sized> {
    Borrowed(&'de T),
    Buffered(&'a T),
    Owned(Box<T>),
}

/// Internal state tracked by the executor/`Visitor`s.
#[derive(Debug)]
struct State<'de> {
    /// Internal operations/expectations not tracked by the public `Visit` and
    /// `Deserialize` steps.
    internal_op: Cell<Option<InternalOp>>,
    visit: Cell<Option<Visit>>,
    deserialize: Cell<Option<Deserialize>>,
    depth: Cell<usize>,
    expected_depth: Cell<usize>,
    data: UnsafeCell<Data<'de>>,
}

/// Borrowed/owned data returned by serde.
enum Data<'de> {
    DeStr(&'de str),
    String(Box<str>),
    Str(NonNull<str>),
    DeBytes(&'de [u8]),
    ByteBuf(Box<[u8]>),
    Bytes(NonNull<[u8]>),
    None
}

/// What the executor expects you to do.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Visit {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(f32),
    F64(f64),
    Char(char),
    Some,
    None,
    Unit,
    Seq,
    Map,
    NewtypeStruct,
    Enum,
    // these are kinda hard
    Str,
    Bytes,
}

/// What you expect the executor to do.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Deserialize {
    Any,
    IgnoredAny,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Char,
    Option,
    Unit,
    Seq,
    Map,
    Identifier,
    Bytes,
    ByteBuf, // decays into Bytes in type resolution
    Str,
    String, // decays into Str in type resolution
    Tuple(usize),
    UnitStruct {
        name: &'static str
    },
    TupleStruct {
        name: &'static str,
        len: usize,
    },
    NewtypeStruct {
        name: &'static str,
    },
    Struct {
        name: &'static str,
        fields: &'static [&'static str],
    },
    Enum {
        name: &'static str,
        variants: &'static [&'static str],
    },
}

/// Internal operations not otherwise exposed in `Visit`/`Deserialize`.
#[derive(Copy, Clone, Debug)]
enum InternalOp {
    /// Ask for a size hint.
    SizeHintReq,
    /// Resulting size hint.
    SizeHintRes(Option<usize>)
}

///// The types which can be deserialized by serde.
//#[derive(Clone, Debug)]
//enum Pack<'de> {
//    Bool(bool),
//    I8(i8),
//    I16(i16),
//    I32(i32),
//    I64(i64),
//    I128(i128),
//    U8(u8),
//    U16(u16),
//    U32(u32),
//    U64(u64),
//    U128(u128),
//    F32(f32),
//    F64(f64),
//    Char(char),
//    Str(Cow<'de, str>),
//    Bytes(Cow<'de, [u8]>),
//    Some(Box<Pack<'de>>),
//    None,
//    Unit,
//    Seq(std::vec::IntoIter<Pack<'de>>),
//    // NOTE: support for multimaps!
//    Map(std::vec::IntoIter<(Pack<'de>, Pack<'de>)>),
//    NewtypeStruct(Box<Pack<'de>>),
//    // NOTE: currently unused!
//    #[allow(unused)]
//    Enum {
//        variant: Box<Pack<'de>>,
//        data: Box<Pack<'de>>,
//    },
//}

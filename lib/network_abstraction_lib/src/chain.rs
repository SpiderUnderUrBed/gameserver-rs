use std::{error::Error, marker::PhantomData, pin::Pin};
use std::any::TypeId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{HandlerType, IntoRequest};

pub struct ChainHere<InnerIdx>(std::marker::PhantomData<InnerIdx>);
pub struct ChainThere<Idx>(std::marker::PhantomData<Idx>);

pub struct ChainsNil;

pub struct ChainsCons<C, Tail> {
    pub(crate) head: C,
    pub(crate) tail: Tail,
}

pub trait FindChain<T, Idx, S> {
    fn find_and_execute(
        &self,
        state: S,
        request: &dyn IntoRequest,
    ) -> impl Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send;
}

// Cant impliment until try_as_dyn or specializations are added
impl<T, C, Tail: Send + Sync, S, InnerIdx> FindChain<T, ChainHere<InnerIdx>, S> for ChainsCons<C, Tail>
where
    C: Execute<S> + Contains<T, InnerIdx> + Sync,
    S: Send,
{
    async fn find_and_execute(
        &self,
        state: S,
        request: &dyn IntoRequest,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        todo!()
        // self.head.execute(state, request).await
    }
}

impl<T, C: Sync, Tail: Sync, S: Send, Idx> FindChain<T, ChainThere<Idx>, S> for ChainsCons<C, Tail>
where
    Tail: FindChain<T, Idx, S>,
{
    async fn find_and_execute(
        &self,
        state: S,
        request: &dyn IntoRequest,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.tail.find_and_execute(state, request).await
    }
}

pub trait Contains<T, Idx> {}

pub trait Execute<S> {
    fn execute(
        &self,
        state: S,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send;
}

pub struct HNil {
}

pub struct HCons<T, F, Tail> {
    _marker: PhantomData<T>,
    f: F,
    tail: Tail,
}

pub struct Here;
pub struct There<Idx>(PhantomData<Idx>);

pub trait InsertOrReplace<T, F, Idx> {
    type Output;
    fn insert_or_replace(&self, f: F) -> Self::Output;
}

impl<T, F> InsertOrReplace<T, F, Here> for HNil {
    type Output = HCons<T, F, HNil>;
    fn insert_or_replace(&self, f: F) -> Self::Output {
        HCons {
            _marker: PhantomData,
            f,
            tail: HNil {},
        }
    }
}

impl<T, F, OldF, Tail: Clone> InsertOrReplace<T, F, Here>
    for HCons<T, OldF, Tail>
{
    type Output = HCons<T, F, Tail>;
    fn insert_or_replace(&self, f: F) -> Self::Output {
        HCons {
            _marker: PhantomData,
            f,
            tail: self.tail.clone(),
        }
    }
}

impl<T, F, Head, HeadF: Clone, Tail, Idx> InsertOrReplace<T, F, There<Idx>>
    for HCons<Head, HeadF, Tail>
where
    Tail: InsertOrReplace<T, F, Idx>,
{
    type Output = HCons<Head, HeadF, Tail::Output>;
    fn insert_or_replace(&self, f: F) -> Self::Output {
        HCons {
            _marker: self._marker,
            f: self.f.clone(),
            tail: self.tail.insert_or_replace(f),
        }
    }
}

pub struct ChainBuilder<H, S> {
    _marker2: PhantomData<S>,
    list: H,
}

impl <S>ChainBuilder<HNil, S> {
    pub fn new() -> Self {
        ChainBuilder {
            list: HNil {},
            _marker2: PhantomData,
        }
    }
}


impl<S: Send> Execute<S> for HNil {
    async fn execute(
        &self,
        _state: S,
        _bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

impl<T: DeserializeOwned + Send + Sync, F, Tail, S: Send> Execute<S> for HCons<T, F, Tail>
where
    F: for<'a> Fn(
        &S,
        T,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>> + Sync,
    Tail: Execute<S> + Send + Sync,
{
    async fn execute(
        &self,
        state: S,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::from_slice(&bytes){
            Ok(output) => {
                (self.f)(&state, output).await?;
            },
            Err(_) => {

            },
        }
        self.tail.execute(state, bytes).await
    }
}

impl<ST, H: Execute<ST>> ChainBuilder<H, ST> {
    pub fn chain<T, F, Idx, S>(&mut self, f: F) -> ChainBuilder<H::Output, S>
    where
        H: InsertOrReplace<T, F, Idx>,
        F: for<'a> Fn(
            S,
            T,
        )
            -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>,
    {
        ChainBuilder {
            list: self.list.insert_or_replace(f),
            _marker2: PhantomData,
        }
    }
    pub async fn decode_bytes(&mut self, state: ST, bytes: Vec<u8>) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.list.execute(state, bytes).await
    }
}


// pub trait CollectTypeIds {
//     fn type_ids(ids: &mut Vec<TypeId>);
// }

// impl CollectTypeIds for HNil {
//     fn type_ids(_ids: &mut Vec<TypeId>) {}
// }

// impl<T: 'static, F, Tail: CollectTypeIds> CollectTypeIds for HCons<T, F, Tail> {
//     fn type_ids(ids: &mut Vec<TypeId>) {
//         ids.push(TypeId::of::<T>());
//         Tail::type_ids(ids);
//     }
// }

// impl<H, S> ChainType<S> for ChainBuilder<H, S>
// where
//     H: Execute<S> + CollectTypeIds + Send + Sync,
//     S: Send + Sync,
// {
//     fn accepts(&self, id: TypeId) -> bool {
//         let mut ids = Vec::new();
//         H::type_ids(&mut ids);
//         ids.contains(&id)
//     }

//     fn execute_erased<'a>(
//         &'a self,
//         state: S,
//         bytes: Vec<u8>,
//     ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'a>> {
//         Box::pin(self.list.execute(state, bytes))
//     }
// }

// trait SerializableRequest: crate::IntoRequest + DeserializeOwned {}

// impl <H: Send + Sync, S: Send + Sync>HandlerType<S> for ChainBuilder<H, S>{
//     fn add_router(&self, router: &crate::Router<S>) {
//         todo!()
//     }

//     fn try_predicate(
//         &mut self,
//         request: &dyn SerializableRequest,
//     ) -> Result<Box<dyn crate::IntoRequest>, crate::RouterErrors> {
//         todo!()
//     }

//     fn get_mapping(&self) -> Option<String> {
//         todo!()
//     }

//     fn mapping(self, mapping: String) -> Self
//     where
//         Self: Sized {
//         todo!()
//     }

//     fn execute<'a>(
//         &mut self,
//         state: &'a S,
//         request: Box<dyn crate::IntoRequest>,
//     ) -> crate::BorrowedBoxFuture<'a, Box<dyn crate::IntoResponse<Box<dyn std::any::Any + Send + Sync>>>> {
//         todo!()
//     }
// }
// //pub fn chain_wrapper_handler()
// // pub fn foo() -> ChainBuilder<HNil> {
// //     ChainBuilder::new()
// // }

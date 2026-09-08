use std::{error::Error, marker::PhantomData, pin::Pin};

use serde::{de::DeserializeOwned, Deserialize, Serialize};


pub trait Execute<S> {
    async fn execute(
        &self,
        state: S,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
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


impl<S> Execute<S> for HNil {
    async fn execute(
        &self,
        _state: S,
        _bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

impl<T: DeserializeOwned, F, Tail, S> Execute<S> for HCons<T, F, Tail>
where
    F: for<'a> Fn(
        T,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>,
    Tail: Execute<S>,
{
    async fn execute(
        &self,
        state: S,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::from_slice(&bytes){
            Ok(output) => {
                (self.f)(output).await?;
            },
            Err(_) => todo!(),
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

// pub fn foo() -> ChainBuilder<HNil> {
//     ChainBuilder::new()
// }

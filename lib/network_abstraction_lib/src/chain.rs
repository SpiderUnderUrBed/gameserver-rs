use std::{error::Error, marker::PhantomData, pin::Pin};

use serde::{de::DeserializeOwned, Deserialize, Serialize};


pub trait Execute {
    async fn execute(
        &self,
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

pub struct ChainBuilder<H> {
    list: H,
}

impl ChainBuilder<HNil> {
    pub fn new() -> Self {
        ChainBuilder {
            list: HNil {},
        }
    }
}


impl Execute for HNil {
    async fn execute(
        &self,
        _bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

impl<T: DeserializeOwned, F, Tail> Execute for HCons<T, F, Tail>
where
    F: for<'a> Fn(
        T,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>,
    Tail: Execute,
{
    async fn execute(
        &self,
        bytes: Vec<u8>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match serde_json::from_slice(&bytes){
            Ok(output) => {
                (self.f)(output).await?;
            },
            Err(_) => todo!(),
        }
        // match T::decode(bytes.clone()) {
        //     Ok(output) => {
               
        //         (self.f)(output).await.map_err(|e| match e {
                  
        //         })?;
        //     }
        //     Err(e) => {
        //     }
        // }
        self.tail.execute(bytes).await
    }
}

impl<H> ChainBuilder<H> {
    pub fn chain<T, F, Idx>(&mut self, f: F) -> ChainBuilder<H::Output>
    where
        H: InsertOrReplace<T, F, Idx>,
        F: for<'a> Fn(
            T,
        )
            -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send>>,
    {
        ChainBuilder {
            list: self.list.insert_or_replace(f),
        }
    }
    pub async fn decode_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
            // let mut total_bytes = Vec::new();
            // total_bytes.extend(self.fs.remainder.clone());
            // self.fs.remainder = Vec::new();
            // total_bytes.extend(bytes);
            // let mut frame = self.fs.state.create_frame_handler();
            // frame.set_chunks(self.fs.remainder.clone());

            // match frame.append_bytes_recv(&total_bytes, remainder) {
            //     Ok(frames) => {
            //         for frame in &frames {
            //             let chunks = frame.get_chunks();
            //             let _ = self
            //                 .list
            //                 .execute(state_id, chunks.clone(), &mut self.fs)
            //                 .await?;
            //         }
            //         if let Some(last_frame) = frames.iter().last() {
            //             self.fs.remainder.extend(last_frame.get_remainder().clone());
            //         }
            //         Ok(())
            //     }
            //     Err(e) => {
                    
            //     }
            // }
    }
    // pub async fn forward(self, bytes: Vec<u8>){

    // }
}

// pub fn foo() -> ChainBuilder<HNil> {
//     ChainBuilder::new()
// }

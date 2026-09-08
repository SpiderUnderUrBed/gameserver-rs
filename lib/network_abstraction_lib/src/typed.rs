use std::sync::{Arc, OnceLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_flexitos::{serialize_trait_object, MapRegistry, Registry as FlexitosRegistry};

use crate::{AsyncFnWrapper, BorrowedBoxFuture};

pub trait TaggedOutput: erased_serde::Serialize + std::fmt::Debug + Send + Sync {
    fn tag(&self) -> &'static str;
}

#[doc(hidden)]
pub struct OutputRegistration {
    pub id: &'static str,
    pub deser: fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn TaggedOutput>>,
}

inventory::collect!(OutputRegistration);

static OUTPUT_REGISTRY: std::sync::Mutex<Option<MapRegistry<dyn TaggedOutput>>> =
    std::sync::Mutex::new(None);

fn output_registry() -> std::sync::MutexGuard<'static, Option<MapRegistry<dyn TaggedOutput>>> {
    let mut guard = OUTPUT_REGISTRY.lock().unwrap();
    if guard.is_none() {
        let mut reg = MapRegistry::<dyn TaggedOutput>::new("TaggedOutput");
        for entry in inventory::iter::<OutputRegistration> {
            reg.register(entry.id, entry.deser);
        }
        *guard = Some(reg);
    }
    guard
}

impl<'a> Serialize for dyn TaggedOutput + 'a {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serialize_trait_object(s, self.tag(), self)
    }
}

impl<'de> Deserialize<'de> for Box<dyn TaggedOutput> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        output_registry().as_mut().unwrap().deserialize_trait_object(d)
    }
}

#[macro_export]
macro_rules! register_output {
    ($ty:ty, $id:expr) => {
        impl $crate::typed::TaggedOutput for $ty {
            fn tag(&self) -> &'static str {
                $id
            }
        }
        $crate::inventory::submit! {
            $crate::typed::OutputRegistration {
                id: $id,
                deser: |d| Ok(Box::new(erased_serde::deserialize::<$ty>(d)?)),
            }
        }
    };
}

pub trait TypedHandler<S>: Send + Sync {
    type Input: DeserializeOwned + Send + Sync + 'static;
    type Output: TaggedOutput + Serialize + Clone + Send + Sync + 'static;

    fn call<'a>(&'a self, state: &'a S, input: Self::Input) -> BorrowedBoxFuture<'a, Self::Output>;
}

pub struct TypedFn<F, In, Out> {
    f: F,
    _marker: std::marker::PhantomData<fn(In) -> Out>,
}

pub fn typed_fn<S, F, In, Out>(f: F) -> TypedFn<F, In, Out>
where
    S: Send + Sync,
    In: DeserializeOwned + Send + Sync + 'static,
    Out: TaggedOutput + Serialize + Clone + Send + Sync + 'static,
    F: for<'a> AsyncFnWrapper<'a, S, In, Output = Out> + Send + Sync,
{
    TypedFn {
        f,
        _marker: std::marker::PhantomData,
    }
}

impl<S, F, In, Out> TypedHandler<S> for TypedFn<F, In, Out>
where
    S: Send + Sync,
    In: DeserializeOwned + Send + Sync + 'static,
    Out: TaggedOutput + Serialize + Clone + Send + Sync + 'static,
    F: for<'a> AsyncFnWrapper<'a, S, In, Output = Out> + Send + Sync,
{
    type Input = In;
    type Output = Out;

    fn call<'a>(&'a self, state: &'a S, input: Self::Input) -> BorrowedBoxFuture<'a, Self::Output> {
        Box::pin(self.f.call(state, input))
    }
}

pub trait RouteInput<S>: DeserializeOwned + Send + Sync + 'static
where
    S: Send + Sync + 'static,
{
    type Output: TaggedOutput + Serialize + Clone + Send + Sync + 'static;

    fn slot() -> &'static OnceLock<Arc<dyn TypedHandler<S, Input = Self, Output = Self::Output>>>;

    fn set(handler: impl TypedHandler<S, Input = Self, Output = Self::Output> + 'static) {
        let _ = Self::slot().set(Arc::new(handler));
    }

    fn get() -> Option<Arc<dyn TypedHandler<S, Input = Self, Output = Self::Output>>> {
        Self::slot().get().cloned()
    }
}
use std::sync::{Arc, OnceLock};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_flexitos::{MapRegistry, Registry as FlexitosRegistry, serialize_trait_object};
use serde_json::Value;

use crate::{AsyncFnWrapper, BorrowedBoxFuture};

use crate::{
    BytesRequest, ErrorResponse, ExtractorErrors, HandlerType, IntoRequest, IntoResponse, Router,
    RouterErrors, ValueRequest,
};
use std::any::Any;


pub struct TypedRequest<I>(pub I);

impl<I> IntoRequest for TypedRequest<I>
where
    I: Clone + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn clone_box(&self) -> Box<dyn IntoRequest> {
        Box::new(TypedRequest(self.0.clone()))
    }
}

pub struct TypedOutputResponse<Out>(pub Out);

impl<Out> IntoResponse<Box<dyn Any + Send + Sync>> for TypedOutputResponse<Out>
where
    Out: TaggedOutput + Serialize + Clone + Send + Sync + 'static,
{
    fn try_into_response(&self) -> Result<Box<dyn Any + Send + Sync>, ExtractorErrors> {
        Ok(Box::new(self.0.clone()) as Box<dyn Any + Send + Sync>)
    }

    fn try_into_bytes(&self) -> Result<Vec<u8>, ExtractorErrors> {
        let tagged: &dyn TaggedOutput = &self.0;
        serde_json::to_vec(tagged).map_err(|e| ExtractorErrors::Err(e.to_string()))
    }
    fn try_into_value(&self) -> Result<Value, ExtractorErrors> {
        let tagged: &dyn TaggedOutput = &self.0;
        serde_json::to_value(tagged).map_err(|e| ExtractorErrors::Err(e.to_string()))
    }
}

pub(crate) struct TypedRouteHandler<S, I> {
    _marker: std::marker::PhantomData<fn(S, I)>,
}

impl<S, I> TypedRouteHandler<S, I> {
    pub(crate) fn new() -> Self {
        TypedRouteHandler {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S, I> HandlerType<S> for TypedRouteHandler<S, I>
where
    S: Send + Sync + 'static,
    I: RouteInput<S> + Clone,
{
    fn add_router(&self, _router: &Router<S>) {}

    fn try_predicate(
        &mut self,
        request: &dyn IntoRequest,
    ) -> Result<Box<dyn IntoRequest>, RouterErrors> {
        if let Some(bytes_req) = request.as_any().downcast_ref::<BytesRequest>() {
            if let Ok(parsed) = serde_json::from_slice::<I>(&bytes_req.bytes) {
                return Ok(Box::new(TypedRequest(parsed)));
            }
        } else if let Some(value_req) = request.as_any().downcast_ref::<ValueRequest>() {
            if let Ok(parsed) = serde_json::from_value::<I>(value_req.value.clone()) {
                return Ok(Box::new(TypedRequest(parsed)));
            }
        }
        Err(RouterErrors::NoHandlerFound)
    }

    fn get_mapping(&self) -> Option<String> {
        None
    }

    fn mapping(self, _mapping: String) -> Self
    where
        Self: Sized,
    {
        self
    }

    fn execute<'a>(
        &mut self,
        state: &'a S,
        request: Box<dyn IntoRequest>,
    ) -> BorrowedBoxFuture<'a, Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>> {
        Box::pin(async move {
            let input = match request.into_any().downcast::<TypedRequest<I>>() {
                Ok(typed) => typed.0,
                Err(_) => {
                    return Box::new(ErrorResponse {
                        error: "typed route handler received the wrong request wrapper".into(),
                    })
                        as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>;
                }
            };

            match I::get() {
                Some(handler) => {
                    let output = handler.call(state, input).await;
                    Box::new(TypedOutputResponse(output))
                        as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>
                }
                None => Box::new(ErrorResponse {
                    error: "no typed handler registered for this input type".into(),
                }) as Box<dyn IntoResponse<Box<dyn Any + Send + Sync>>>,
            }
        })
    }
}

pub trait TaggedOutput: erased_serde::Serialize + std::fmt::Debug + Send + Sync {
    fn tag(&self) -> &'static str;
}

#[doc(hidden)]
pub struct OutputRegistration {
    pub id: &'static str,
    pub deser:
        fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn TaggedOutput>>,
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
        output_registry()
            .as_mut()
            .unwrap()
            .deserialize_trait_object(d)
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
                deser: |d| Ok(Box::new($crate::erased_serde::deserialize::<$ty>(d)?)),
            }
        }
    };
}

#[macro_export]
macro_rules! register_output_single {
    ($ty:ident) => {
        $crate::register_output!($ty, stringify!($ty));
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

    fn type_key() -> String {
        std::any::type_name::<Self>().to_string()
    }

    fn set(handler: impl TypedHandler<S, Input = Self, Output = Self::Output> + 'static) {
        let _ = Self::slot().set(Arc::new(handler));
    }

    fn get() -> Option<Arc<dyn TypedHandler<S, Input = Self, Output = Self::Output>>> {
        Self::slot().get().cloned()
    }
}

impl<T> TaggedOutput for Option<T>
where
    T: TaggedOutput + Serialize,
{
    fn tag(&self) -> &'static str {
        match self {
            Some(inner) => inner.tag(),
            None => "None",
        }
    }
}

impl<T, E> TaggedOutput for Result<T, E>
where
    T: TaggedOutput + Serialize,
    E: TaggedOutput + Serialize,
{
    fn tag(&self) -> &'static str {
        match self {
            Ok(inner) => inner.tag(),
            Err(inner) => inner.tag(),
        }
    }
}
//! Resource management

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use crate::{
    core::{
        parking_lot::MutexGuard,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    state::ResourceState,
    untyped::UntypedResource,
};
use fxhash::FxHashSet;
use std::{
    any::Any,
    error::Error,
    fmt::{Debug, Formatter},
    future::Future,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};

use crate::state::LoadError;
pub use fyrox_core as core;

pub mod constructor;
pub mod entry;
pub mod event;
pub mod graph;
pub mod io;
pub mod loader;
pub mod manager;
pub mod options;
pub mod state;
mod task;
pub mod untyped;

/// Type UUID of texture resource. It is defined here to load old versions of resources.
pub const TEXTURE_RESOURCE_UUID: Uuid = uuid!("02c23a44-55fa-411a-bc39-eb7a5eadf15c");
/// Type UUID of model resource. It is defined here to load old versions of resources.
pub const MODEL_RESOURCE_UUID: Uuid = uuid!("44cd768f-b4ca-4804-a98c-0adf85577ada");
/// Type UUID of sound buffer resource. It is defined here to load old versions of resources.
pub const SOUND_BUFFER_RESOURCE_UUID: Uuid = uuid!("f6a077b7-c8ff-4473-a95b-0289441ea9d8");
/// Type UUID of shader resource. It is defined here to load old versions of resources.
pub const SHADER_RESOURCE_UUID: Uuid = uuid!("f1346417-b726-492a-b80f-c02096c6c019");
/// Type UUID of curve resource. It is defined here to load old versions of resources.
pub const CURVE_RESOURCE_UUID: Uuid = uuid!("f28b949f-28a2-4b68-9089-59c234f58b6b");

/// A trait for resource data.
pub trait ResourceData: 'static + Debug + Visit + Send + Reflect {
    /// Returns path of resource data.
    fn path(&self) -> &Path;

    /// Sets new path to resource data.
    fn set_path(&mut self, path: PathBuf);

    /// Returns `self` as `&dyn Any`. It is useful to implement downcasting to a particular type.
    fn as_any(&self) -> &dyn Any;

    /// Returns `self` as `&mut dyn Any`. It is useful to implement downcasting to a particular type.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Returns unique data type id.
    fn type_uuid(&self) -> Uuid;

    /// Returns true if the resource data was generated procedurally, not taken from a file.
    fn is_embedded(&self) -> bool;

    /// Saves the resource data a file at the specified path. By default, this method returns an
    /// error that tells that saving functionality is not implemented. This method is free to
    /// decide how the resource data is saved. This is needed, because there are multiple formats
    /// that defines various kinds of resources. For example, a rectangular texture could be saved
    /// into a whole bunch of formats, such as png, bmp, tga, jpg etc, but in the engine it is single
    /// Texture resource. In any case, produced file should be compatible with a respective resource
    /// loader.
    fn save(&mut self, #[allow(unused_variables)] path: &Path) -> Result<(), Box<dyn Error>> {
        Err("Saving is not supported!".to_string().into())
    }
}

/// Extension trait for a resource data of a particular type, which adds additional functionality,
/// such as: a way to get default state of the data (`Default` impl), a way to get data's type uuid.
/// The trait has automatic implementation for any type that implements
/// ` ResourceData + Default + TypeUuidProvider` traits.
pub trait TypedResourceData: ResourceData + Default + TypeUuidProvider {}

impl<T> TypedResourceData for T where T: ResourceData + Default + TypeUuidProvider {}

/// A trait for resource load error.
pub trait ResourceLoadError: 'static + Debug + Send + Sync {}

impl<T> ResourceLoadError for T where T: 'static + Debug + Send + Sync {}

/// Provides typed access to a resource state.
pub struct ResourceStateGuard<'a, T>
where
    T: TypedResourceData,
{
    guard: MutexGuard<'a, ResourceState>,
    phantom: PhantomData<T>,
}

impl<'a, T> ResourceStateGuard<'a, T>
where
    T: TypedResourceData,
{
    /// Fetches the actual state of the resource.
    pub fn get(&self) -> ResourceStateRef<'_, T> {
        match &*self.guard {
            ResourceState::Pending {
                path, type_uuid, ..
            } => ResourceStateRef::Pending {
                path,
                type_uuid: *type_uuid,
            },
            ResourceState::LoadError {
                path,
                error,
                type_uuid,
            } => ResourceStateRef::LoadError {
                path,
                error,
                type_uuid: *type_uuid,
            },
            ResourceState::Ok(data) => ResourceStateRef::Ok(
                ResourceData::as_any(&**data)
                    .downcast_ref()
                    .expect("Type mismatch!"),
            ),
        }
    }

    /// Fetches the actual state of the resource.
    pub fn get_mut(&mut self) -> ResourceStateRefMut<'_, T> {
        match &mut *self.guard {
            ResourceState::Pending {
                path, type_uuid, ..
            } => ResourceStateRefMut::Pending {
                path,
                type_uuid: *type_uuid,
            },
            ResourceState::LoadError {
                path,
                error,
                type_uuid,
            } => ResourceStateRefMut::LoadError {
                path,
                error,
                type_uuid: *type_uuid,
            },
            ResourceState::Ok(data) => ResourceStateRefMut::Ok(
                ResourceData::as_any_mut(&mut **data)
                    .downcast_mut()
                    .expect("Type mismatch!"),
            ),
        }
    }
}

/// Provides typed access to a resource state.
#[derive(Debug)]
pub enum ResourceStateRef<'a, T>
where
    T: TypedResourceData,
{
    /// Resource is loading from external resource or in the queue to load.
    Pending {
        /// A path to load resource from.
        path: &'a PathBuf,
        /// Actual resource type id.
        type_uuid: Uuid,
    },
    /// An error has occurred during the load.
    LoadError {
        /// A path at which it was impossible to load the resource.
        path: &'a PathBuf,
        /// An error.
        error: &'a LoadError,
        /// Actual resource type id.
        type_uuid: Uuid,
    },
    /// Actual resource data when it is fully loaded.
    Ok(&'a T),
}

/// Provides typed access to a resource state.
#[derive(Debug)]
pub enum ResourceStateRefMut<'a, T> {
    /// Resource is loading from external resource or in the queue to load.
    Pending {
        /// A path to load resource from.
        path: &'a mut PathBuf,
        /// Actual resource type id.
        type_uuid: Uuid,
    },
    /// An error has occurred during the load.
    LoadError {
        /// A path at which it was impossible to load the resource.
        path: &'a mut PathBuf,
        /// An error.
        error: &'a mut LoadError,
        /// Actual resource type id.
        type_uuid: Uuid,
    },
    /// Actual resource data when it is fully loaded.
    Ok(&'a mut T),
}

/// A resource of particular data type. It is a typed wrapper around [`UntypedResource`] which
/// does type checks at runtime.
///
/// ## Default State
///
/// Default state of the resource will be [`ResourceState::Ok`] with `T::default`.
#[derive(Debug, Reflect)]
pub struct Resource<T>
where
    T: TypedResourceData,
{
    untyped: UntypedResource,
    #[reflect(hidden)]
    phantom: PhantomData<T>,
}

impl<T> Visit for Resource<T>
where
    T: TypedResourceData,
{
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut region = visitor.enter_region(name)?;

        // Backward compatibility.
        if region.is_reading() {
            let mut old_option_wrapper: Option<UntypedResource> = None;
            if old_option_wrapper.visit("State", &mut region).is_ok() {
                self.untyped = old_option_wrapper.unwrap();
            } else {
                self.untyped.visit("State", &mut region)?;
            }
        } else {
            self.untyped.visit("State", &mut region)?;
        }

        Ok(())
    }
}

impl<T> PartialEq for Resource<T>
where
    T: TypedResourceData,
{
    fn eq(&self, other: &Self) -> bool {
        self.untyped == other.untyped
    }
}

impl<T> Eq for Resource<T> where T: TypedResourceData {}

impl<T> Hash for Resource<T>
where
    T: TypedResourceData,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.untyped.hash(state)
    }
}

impl<T> Resource<T>
where
    T: TypedResourceData,
{
    /// Creates new resource in pending state.
    #[inline]
    pub fn new_pending(path: PathBuf) -> Self {
        Self {
            untyped: UntypedResource::new_pending(path, <T as TypeUuidProvider>::type_uuid()),
            phantom: PhantomData,
        }
    }

    /// Creates new resource in ok state (fully loaded).
    #[inline]
    pub fn new_ok(data: T) -> Self {
        Self {
            untyped: UntypedResource::new_ok(data),
            phantom: PhantomData,
        }
    }

    /// Creates new resource in error state.
    #[inline]
    pub fn new_load_error(path: PathBuf, error: LoadError) -> Self {
        Self {
            untyped: UntypedResource::new_load_error(
                path,
                error,
                <T as TypeUuidProvider>::type_uuid(),
            ),
            phantom: PhantomData,
        }
    }

    /// Converts self to internal value.
    #[inline]
    pub fn into_untyped(self) -> UntypedResource {
        self.untyped
    }

    /// Locks internal mutex provides access to the state.
    #[inline]
    pub fn state(&self) -> ResourceStateGuard<'_, T> {
        ResourceStateGuard {
            guard: self.state_inner(),
            phantom: Default::default(),
        }
    }

    /// Tries to lock internal mutex provides access to the state.
    #[inline]
    pub fn try_acquire_state(&self) -> Option<ResourceStateGuard<'_, T>> {
        self.untyped.0.try_lock().map(|guard| ResourceStateGuard {
            guard,
            phantom: Default::default(),
        })
    }

    fn state_inner(&self) -> MutexGuard<'_, ResourceState> {
        self.untyped.0.lock()
    }

    /// Returns true if the resource is still loading.
    #[inline]
    pub fn is_loading(&self) -> bool {
        matches!(*self.state_inner(), ResourceState::Pending { .. })
    }

    /// Returns true if the resource is fully loaded and ready for use.
    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(*self.state_inner(), ResourceState::Ok(_))
    }

    /// Returns true if the resource is failed to load.
    #[inline]
    pub fn is_failed_to_load(&self) -> bool {
        matches!(*self.state_inner(), ResourceState::LoadError { .. })
    }

    /// Returns exact amount of users of the resource.
    #[inline]
    pub fn use_count(&self) -> usize {
        self.untyped.use_count()
    }

    /// Returns a pointer as numeric value which can be used as a hash.
    #[inline]
    pub fn key(&self) -> usize {
        self.untyped.key()
    }

    /// Returns path of the resource.
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.untyped.0.lock().path().to_path_buf()
    }

    /// Sets a new path of the resource.
    #[inline]
    pub fn set_path(&mut self, new_path: PathBuf) {
        self.untyped.set_path(new_path);
    }

    /// Allows you to obtain reference to the resource data.
    ///
    /// # Panic
    ///
    /// An attempt to use method result will panic if resource is not loaded yet, or
    /// there was load error. Usually this is ok because normally you'd chain this call
    /// like this `resource.await?.data_ref()`. Every resource implements Future trait
    /// and it returns Result, so if you'll await future then you'll get Result, so
    /// call to `data_ref` will be fine.
    #[inline]
    pub fn data_ref(&self) -> ResourceDataRef<'_, T> {
        ResourceDataRef {
            guard: self.state_inner(),
            phantom: Default::default(),
        }
    }
}

impl<T> Default for Resource<T>
where
    T: TypedResourceData,
{
    #[inline]
    fn default() -> Self {
        Self {
            untyped: UntypedResource::new_ok(T::default()),
            phantom: Default::default(),
        }
    }
}

impl<T> Clone for Resource<T>
where
    T: TypedResourceData,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            phantom: Default::default(),
        }
    }
}

impl<T> From<UntypedResource> for Resource<T>
where
    T: TypedResourceData,
{
    #[inline]
    fn from(untyped: UntypedResource) -> Self {
        assert_eq!(untyped.type_uuid(), <T as TypeUuidProvider>::type_uuid());
        Self {
            untyped,
            phantom: Default::default(),
        }
    }
}

#[allow(clippy::from_over_into)]
impl<T> Into<UntypedResource> for Resource<T>
where
    T: TypedResourceData,
{
    #[inline]
    fn into(self) -> UntypedResource {
        self.untyped
    }
}

impl<T> Future for Resource<T>
where
    T: TypedResourceData,
{
    type Output = Result<Self, LoadError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.untyped.clone();
        Pin::new(&mut inner)
            .poll(cx)
            .map(|r| r.map(|_| self.clone()))
    }
}

#[doc(hidden)]
pub struct ResourceDataRef<'a, T>
where
    T: TypedResourceData,
{
    guard: MutexGuard<'a, ResourceState>,
    phantom: PhantomData<T>,
}

impl<'a, T> Debug for ResourceDataRef<'a, T>
where
    T: TypedResourceData,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self.guard {
            ResourceState::Pending { ref path, .. } => {
                write!(
                    f,
                    "Attempt to get reference to resource data while it is not loaded! Path is {}",
                    path.display()
                )
            }
            ResourceState::LoadError { ref path, .. } => {
                write!(
                    f,
                    "Attempt to get reference to resource data which failed to load! Path is {}",
                    path.display()
                )
            }
            ResourceState::Ok(ref data) => data.fmt(f),
        }
    }
}

impl<'a, T> Deref for ResourceDataRef<'a, T>
where
    T: TypedResourceData,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match *self.guard {
            ResourceState::Pending { ref path, .. } => {
                panic!(
                    "Attempt to get reference to resource data while it is not loaded! Path is {}",
                    path.display()
                )
            }
            ResourceState::LoadError { ref path, .. } => {
                panic!(
                    "Attempt to get reference to resource data which failed to load! Path is {}",
                    path.display()
                )
            }
            ResourceState::Ok(ref data) => ResourceData::as_any(&**data)
                .downcast_ref()
                .expect("Type mismatch!"),
        }
    }
}

impl<'a, T> DerefMut for ResourceDataRef<'a, T>
where
    T: TypedResourceData,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match *self.guard {
            ResourceState::Pending { ref path, .. } => {
                panic!(
                    "Attempt to get reference to resource data while it is not loaded! Path is {}",
                    path.display()
                )
            }
            ResourceState::LoadError { ref path, .. } => {
                panic!(
                    "Attempt to get reference to resource data which failed to load! Path is {}",
                    path.display()
                )
            }
            ResourceState::Ok(ref mut data) => ResourceData::as_any_mut(&mut **data)
                .downcast_mut()
                .expect("Type mismatch!"),
        }
    }
}

/// Collects all resources used by a given entity. Internally, it uses reflection to iterate over
/// each field of every descendant sub-object of the entity. This function could be used to collect
/// all resources used by an object, which could be useful if you're building a resource dependency
/// analyzer.
pub fn collect_used_resources(
    entity: &dyn Reflect,
    resources_collection: &mut FxHashSet<UntypedResource>,
) {
    #[inline(always)]
    fn type_is<T: Reflect>(entity: &dyn Reflect) -> bool {
        let mut types_match = false;
        entity.downcast_ref::<T>(&mut |v| {
            types_match = v.is_some();
        });
        types_match
    }

    // Skip potentially large chunks of numeric data, that definitely cannot contain any resources.
    // TODO: This is a brute-force solution which does not include all potential types with plain
    // data.
    let mut finished = type_is::<Vec<u8>>(entity)
        || type_is::<Vec<u16>>(entity)
        || type_is::<Vec<u32>>(entity)
        || type_is::<Vec<u64>>(entity)
        || type_is::<Vec<i8>>(entity)
        || type_is::<Vec<i16>>(entity)
        || type_is::<Vec<i32>>(entity)
        || type_is::<Vec<i64>>(entity)
        || type_is::<Vec<f32>>(entity)
        || type_is::<Vec<f64>>(entity);

    if finished {
        return;
    }

    entity.downcast_ref::<UntypedResource>(&mut |v| {
        if let Some(resource) = v {
            resources_collection.insert(resource.clone());
            finished = true;
        }
    });

    if finished {
        return;
    }

    entity.as_array(&mut |array| {
        if let Some(array) = array {
            for i in 0..array.reflect_len() {
                if let Some(item) = array.reflect_index(i) {
                    collect_used_resources(item, resources_collection)
                }
            }

            finished = true;
        }
    });

    if finished {
        return;
    }

    entity.as_inheritable_variable(&mut |inheritable| {
        if let Some(inheritable) = inheritable {
            collect_used_resources(inheritable.inner_value_ref(), resources_collection);

            finished = true;
        }
    });

    if finished {
        return;
    }

    entity.as_hash_map(&mut |hash_map| {
        if let Some(hash_map) = hash_map {
            for i in 0..hash_map.reflect_len() {
                if let Some((key, value)) = hash_map.reflect_get_at(i) {
                    collect_used_resources(key, resources_collection);
                    collect_used_resources(value, resources_collection);
                }
            }

            finished = true;
        }
    });

    if finished {
        return;
    }

    entity.fields(&mut |fields| {
        for field in fields {
            collect_used_resources(*field, resources_collection);
        }
    })
}

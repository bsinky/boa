//! This module implements the Rust representation of a JavaScript object.

use crate::{
    builtins::{
        array::array_iterator::ArrayIterator,
        array_buffer::ArrayBuffer,
        function::arguments::{Arguments, MappedArguments},
        function::{BoundFunction, Captures, Function, NativeFunctionSignature},
        map::map_iterator::MapIterator,
        map::ordered_map::OrderedMap,
        object::for_in_iterator::ForInIterator,
        regexp::regexp_string_iterator::RegExpStringIterator,
        set::ordered_set::OrderedSet,
        set::set_iterator::SetIterator,
        string::string_iterator::StringIterator,
        typed_array::integer_indexed_object::IntegerIndexed,
        Date, RegExp,
    },
    context::StandardConstructor,
    gc::{Finalize, Trace},
    property::{Attribute, PropertyDescriptor, PropertyKey},
    Context, JsBigInt, JsResult, JsString, JsSymbol, JsValue,
};
use std::{
    any::Any,
    fmt::{self, Debug, Display},
    ops::{Deref, DerefMut},
};

pub use jsobject::{JsObject, RecursionLimiter, Ref, RefMut};
pub use operations::IntegrityLevel;
pub use property_map::*;

use self::internal_methods::{
    arguments::ARGUMENTS_EXOTIC_INTERNAL_METHODS,
    array::ARRAY_EXOTIC_INTERNAL_METHODS,
    bound_function::{
        BOUND_CONSTRUCTOR_EXOTIC_INTERNAL_METHODS, BOUND_FUNCTION_EXOTIC_INTERNAL_METHODS,
    },
    function::{CONSTRUCTOR_INTERNAL_METHODS, FUNCTION_INTERNAL_METHODS},
    integer_indexed::INTEGER_INDEXED_EXOTIC_INTERNAL_METHODS,
    string::STRING_EXOTIC_INTERNAL_METHODS,
    InternalObjectMethods, ORDINARY_INTERNAL_METHODS,
};

#[cfg(test)]
mod tests;

pub(crate) mod internal_methods;
mod jsobject;
mod operations;
mod property_map;

/// Static `prototype`, usually set on constructors as a key to point to their respective prototype object.
pub static PROTOTYPE: &str = "prototype";

pub type JsPrototype = Option<JsObject>;

/// This trait allows Rust types to be passed around as objects.
///
/// This is automatically implemented, when a type implements `Debug`, `Any` and `Trace`.
pub trait NativeObject: Debug + Any + Trace {
    /// Convert the Rust type which implements `NativeObject` to a `&dyn Any`.
    fn as_any(&self) -> &dyn Any;

    /// Convert the Rust type which implements `NativeObject` to a `&mut dyn Any`.
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

impl<T: Any + Debug + Trace> NativeObject for T {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    #[inline]
    fn as_mut_any(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }
}

/// The internal representation of a JavaScript object.
#[derive(Debug, Trace, Finalize)]
pub struct Object {
    /// The type of the object.
    pub data: ObjectData,
    /// The collection of properties contained in the object
    properties: PropertyMap,
    /// Instance prototype `__proto__`.
    prototype: JsPrototype,
    /// Whether it can have new properties added to it.
    extensible: bool,
}

/// Defines the kind of an object and its internal methods
#[derive(Trace, Finalize)]
pub struct ObjectData {
    kind: ObjectKind,
    internal_methods: &'static InternalObjectMethods,
}

/// Defines the different types of objects.
#[derive(Debug, Trace, Finalize)]
pub enum ObjectKind {
    Array,
    ArrayIterator(ArrayIterator),
    ArrayBuffer(ArrayBuffer),
    Map(OrderedMap<JsValue>),
    MapIterator(MapIterator),
    RegExp(Box<RegExp>),
    RegExpStringIterator(RegExpStringIterator),
    BigInt(JsBigInt),
    Boolean(bool),
    ForInIterator(ForInIterator),
    Function(Function),
    BoundFunction(BoundFunction),
    Set(OrderedSet<JsValue>),
    SetIterator(SetIterator),
    String(JsString),
    StringIterator(StringIterator),
    Number(f64),
    Symbol(JsSymbol),
    Error,
    Ordinary,
    Date(Date),
    Global,
    Arguments(Arguments),
    NativeObject(Box<dyn NativeObject>),
    IntegerIndexed(IntegerIndexed),
}

impl ObjectData {
    /// Create the `Array` object data and reference its exclusive internal methods
    pub fn array() -> Self {
        Self {
            kind: ObjectKind::Array,
            internal_methods: &ARRAY_EXOTIC_INTERNAL_METHODS,
        }
    }

    /// Create the `ArrayIterator` object data
    pub fn array_iterator(array_iterator: ArrayIterator) -> Self {
        Self {
            kind: ObjectKind::ArrayIterator(array_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `ArrayBuffer` object data
    pub fn array_buffer(array_buffer: ArrayBuffer) -> Self {
        Self {
            kind: ObjectKind::ArrayBuffer(array_buffer),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Map` object data
    pub fn map(map: OrderedMap<JsValue>) -> Self {
        Self {
            kind: ObjectKind::Map(map),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `MapIterator` object data
    pub fn map_iterator(map_iterator: MapIterator) -> Self {
        Self {
            kind: ObjectKind::MapIterator(map_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `RegExp` object data
    pub fn reg_exp(reg_exp: Box<RegExp>) -> Self {
        Self {
            kind: ObjectKind::RegExp(reg_exp),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `RegExpStringIterator` object data
    pub fn reg_exp_string_iterator(reg_exp_string_iterator: RegExpStringIterator) -> Self {
        Self {
            kind: ObjectKind::RegExpStringIterator(reg_exp_string_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `BigInt` object data
    pub fn big_int(big_int: JsBigInt) -> Self {
        Self {
            kind: ObjectKind::BigInt(big_int),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Boolean` object data
    pub fn boolean(boolean: bool) -> Self {
        Self {
            kind: ObjectKind::Boolean(boolean),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `ForInIterator` object data
    pub fn for_in_iterator(for_in_iterator: ForInIterator) -> Self {
        Self {
            kind: ObjectKind::ForInIterator(for_in_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Function` object data
    pub fn function(function: Function) -> Self {
        Self {
            internal_methods: if function.is_constructor() {
                &CONSTRUCTOR_INTERNAL_METHODS
            } else {
                &FUNCTION_INTERNAL_METHODS
            },
            kind: ObjectKind::Function(function),
        }
    }

    /// Create the `BoundFunction` object data
    pub fn bound_function(bound_function: BoundFunction, constructor: bool) -> Self {
        Self {
            kind: ObjectKind::BoundFunction(bound_function),
            internal_methods: if constructor {
                &BOUND_CONSTRUCTOR_EXOTIC_INTERNAL_METHODS
            } else {
                &BOUND_FUNCTION_EXOTIC_INTERNAL_METHODS
            },
        }
    }

    /// Create the `Set` object data
    pub fn set(set: OrderedSet<JsValue>) -> Self {
        Self {
            kind: ObjectKind::Set(set),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `SetIterator` object data
    pub fn set_iterator(set_iterator: SetIterator) -> Self {
        Self {
            kind: ObjectKind::SetIterator(set_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `String` object data and reference its exclusive internal methods
    pub fn string(string: JsString) -> Self {
        Self {
            kind: ObjectKind::String(string),
            internal_methods: &STRING_EXOTIC_INTERNAL_METHODS,
        }
    }

    /// Create the `StringIterator` object data
    pub fn string_iterator(string_iterator: StringIterator) -> Self {
        Self {
            kind: ObjectKind::StringIterator(string_iterator),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Number` object data
    pub fn number(number: f64) -> Self {
        Self {
            kind: ObjectKind::Number(number),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Symbol` object data
    pub fn symbol(symbol: JsSymbol) -> Self {
        Self {
            kind: ObjectKind::Symbol(symbol),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Error` object data
    pub fn error() -> Self {
        Self {
            kind: ObjectKind::Error,
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Ordinary` object data
    pub fn ordinary() -> Self {
        Self {
            kind: ObjectKind::Ordinary,
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Date` object data
    pub fn date(date: Date) -> Self {
        Self {
            kind: ObjectKind::Date(date),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Global` object data
    pub fn global() -> Self {
        Self {
            kind: ObjectKind::Global,
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Create the `Arguments` object data
    pub fn arguments(arguments: Arguments) -> Self {
        Self {
            internal_methods: if matches!(arguments, Arguments::Unmapped) {
                &ORDINARY_INTERNAL_METHODS
            } else {
                &ARGUMENTS_EXOTIC_INTERNAL_METHODS
            },
            kind: ObjectKind::Arguments(arguments),
        }
    }

    /// Create the `NativeObject` object data
    pub fn native_object(native_object: Box<dyn NativeObject>) -> Self {
        Self {
            kind: ObjectKind::NativeObject(native_object),
            internal_methods: &ORDINARY_INTERNAL_METHODS,
        }
    }

    /// Creates the `IntegerIndexed` object data
    pub fn integer_indexed(integer_indexed: IntegerIndexed) -> Self {
        Self {
            kind: ObjectKind::IntegerIndexed(integer_indexed),
            internal_methods: &INTEGER_INDEXED_EXOTIC_INTERNAL_METHODS,
        }
    }
}

impl Display for ObjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Array => "Array",
            Self::ArrayIterator(_) => "ArrayIterator",
            Self::ArrayBuffer(_) => "ArrayBuffer",
            Self::ForInIterator(_) => "ForInIterator",
            Self::Function(_) => "Function",
            Self::BoundFunction(_) => "BoundFunction",
            Self::RegExp(_) => "RegExp",
            Self::RegExpStringIterator(_) => "RegExpStringIterator",
            Self::Map(_) => "Map",
            Self::MapIterator(_) => "MapIterator",
            Self::Set(_) => "Set",
            Self::SetIterator(_) => "SetIterator",
            Self::String(_) => "String",
            Self::StringIterator(_) => "StringIterator",
            Self::Symbol(_) => "Symbol",
            Self::Error => "Error",
            Self::Ordinary => "Ordinary",
            Self::Boolean(_) => "Boolean",
            Self::Number(_) => "Number",
            Self::BigInt(_) => "BigInt",
            Self::Date(_) => "Date",
            Self::Global => "Global",
            Self::Arguments(_) => "Arguments",
            Self::NativeObject(_) => "NativeObject",
            Self::IntegerIndexed(_) => "TypedArray",
        })
    }
}

impl Debug for ObjectData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObjectData")
            .field("kind", &self.kind)
            .field("internal_methods", &"internal_methods")
            .finish()
    }
}

impl Default for Object {
    /// Return a new ObjectData struct, with `kind` set to Ordinary
    #[inline]
    fn default() -> Self {
        Self {
            data: ObjectData::ordinary(),
            properties: PropertyMap::default(),
            prototype: None,
            extensible: true,
        }
    }
}

impl Object {
    #[inline]
    pub fn kind(&self) -> &ObjectKind {
        &self.data.kind
    }

    /// Checks if it an `Array` object.
    #[inline]
    pub fn is_array(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Array,
                ..
            }
        )
    }

    #[inline]
    pub fn as_array(&self) -> Option<()> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Array,
                ..
            } => Some(()),
            _ => None,
        }
    }

    /// Checks if it is an `ArrayIterator` object.
    #[inline]
    pub fn is_array_iterator(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::ArrayIterator(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_array_iterator(&self) -> Option<&ArrayIterator> {
        match self.data {
            ObjectData {
                kind: ObjectKind::ArrayIterator(ref iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    /// Checks if it an `ArrayBuffer` object.
    #[inline]
    pub fn is_array_buffer(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::ArrayBuffer(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_array_buffer(&self) -> Option<&ArrayBuffer> {
        match &self.data {
            ObjectData {
                kind: ObjectKind::ArrayBuffer(buffer),
                ..
            } => Some(buffer),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_buffer_mut(&mut self) -> Option<&mut ArrayBuffer> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::ArrayBuffer(buffer),
                ..
            } => Some(buffer),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_iterator_mut(&mut self) -> Option<&mut ArrayIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::ArrayIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn as_string_iterator_mut(&mut self) -> Option<&mut StringIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::StringIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn as_regexp_string_iterator_mut(&mut self) -> Option<&mut RegExpStringIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::RegExpStringIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn as_for_in_iterator(&self) -> Option<&ForInIterator> {
        match &self.data {
            ObjectData {
                kind: ObjectKind::ForInIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn as_for_in_iterator_mut(&mut self) -> Option<&mut ForInIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::ForInIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    /// Checks if it is a `Map` object.pub
    #[inline]
    pub fn is_map(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Map(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_map_ref(&self) -> Option<&OrderedMap<JsValue>> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Map(ref map),
                ..
            } => Some(map),
            _ => None,
        }
    }

    #[inline]
    pub fn as_map_mut(&mut self) -> Option<&mut OrderedMap<JsValue>> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::Map(map),
                ..
            } => Some(map),
            _ => None,
        }
    }

    #[inline]
    pub fn is_map_iterator(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::MapIterator(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_map_iterator_ref(&self) -> Option<&MapIterator> {
        match &self.data {
            ObjectData {
                kind: ObjectKind::MapIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn as_map_iterator_mut(&mut self) -> Option<&mut MapIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::MapIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Set(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_set_ref(&self) -> Option<&OrderedSet<JsValue>> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Set(ref set),
                ..
            } => Some(set),
            _ => None,
        }
    }

    #[inline]
    pub fn as_set_mut(&mut self) -> Option<&mut OrderedSet<JsValue>> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::Set(set),
                ..
            } => Some(set),
            _ => None,
        }
    }

    #[inline]
    pub fn as_set_iterator_mut(&mut self) -> Option<&mut SetIterator> {
        match &mut self.data {
            ObjectData {
                kind: ObjectKind::SetIterator(iter),
                ..
            } => Some(iter),
            _ => None,
        }
    }

    /// Checks if it a `String` object.
    #[inline]
    pub fn is_string(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::String(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_string(&self) -> Option<JsString> {
        match self.data {
            ObjectData {
                kind: ObjectKind::String(ref string),
                ..
            } => Some(string.clone()),
            _ => None,
        }
    }

    /// Checks if it a `Function` object.
    #[inline]
    pub fn is_function(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Function(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_function(&self) -> Option<&Function> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Function(ref function),
                ..
            } => Some(function),
            _ => None,
        }
    }

    #[inline]
    pub fn as_function_mut(&mut self) -> Option<&mut Function> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Function(ref mut function),
                ..
            } => Some(function),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bound_function(&self) -> Option<&BoundFunction> {
        match self.data {
            ObjectData {
                kind: ObjectKind::BoundFunction(ref bound_function),
                ..
            } => Some(bound_function),
            _ => None,
        }
    }

    /// Checks if it a Symbol object.
    #[inline]
    pub fn is_symbol(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Symbol(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_symbol(&self) -> Option<JsSymbol> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Symbol(ref symbol),
                ..
            } => Some(symbol.clone()),
            _ => None,
        }
    }

    /// Checks if it an Error object.
    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Error,
                ..
            }
        )
    }

    #[inline]
    pub fn as_error(&self) -> Option<()> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Error,
                ..
            } => Some(()),
            _ => None,
        }
    }

    /// Checks if it a Boolean object.
    #[inline]
    pub fn is_boolean(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Boolean(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_boolean(&self) -> Option<bool> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Boolean(boolean),
                ..
            } => Some(boolean),
            _ => None,
        }
    }

    /// Checks if it a `Number` object.
    #[inline]
    pub fn is_number(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Number(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_number(&self) -> Option<f64> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Number(number),
                ..
            } => Some(number),
            _ => None,
        }
    }

    /// Checks if it a `BigInt` object.
    #[inline]
    pub fn is_bigint(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::BigInt(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_bigint(&self) -> Option<&JsBigInt> {
        match self.data {
            ObjectData {
                kind: ObjectKind::BigInt(ref bigint),
                ..
            } => Some(bigint),
            _ => None,
        }
    }

    #[inline]
    pub fn is_date(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Date(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_date(&self) -> Option<&Date> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Date(ref date),
                ..
            } => Some(date),
            _ => None,
        }
    }

    /// Checks if it a `RegExp` object.
    #[inline]
    pub fn is_regexp(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::RegExp(_),
                ..
            }
        )
    }

    /// Gets the regexp data if the object is a regexp.
    #[inline]
    pub fn as_regexp(&self) -> Option<&RegExp> {
        match self.data {
            ObjectData {
                kind: ObjectKind::RegExp(ref regexp),
                ..
            } => Some(regexp),
            _ => None,
        }
    }

    /// Checks if it a `TypedArray` object.
    #[inline]
    pub fn is_typed_array(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::IntegerIndexed(_),
                ..
            }
        )
    }

    /// Checks if it is an `Arguments` object.
    #[inline]
    pub fn is_arguments(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Arguments(_),
                ..
            }
        )
    }

    /// Gets the mapped arguments data if this is a mapped arguments object.
    #[inline]
    pub fn as_mapped_arguments(&self) -> Option<&MappedArguments> {
        match self.data {
            ObjectData {
                kind: ObjectKind::Arguments(Arguments::Mapped(ref args)),
                ..
            } => Some(args),
            _ => None,
        }
    }

    /// Gets the typed array data (integer indexed object) if this is a typed array.
    #[inline]
    pub fn as_typed_array(&self) -> Option<&IntegerIndexed> {
        match self.data {
            ObjectData {
                kind: ObjectKind::IntegerIndexed(ref integer_indexed_object),
                ..
            } => Some(integer_indexed_object),
            _ => None,
        }
    }

    /// Gets the typed array data (integer indexed object) if this is a typed array.
    #[inline]
    pub fn as_typed_array_mut(&mut self) -> Option<&mut IntegerIndexed> {
        match self.data {
            ObjectData {
                kind: ObjectKind::IntegerIndexed(ref mut integer_indexed_object),
                ..
            } => Some(integer_indexed_object),
            _ => None,
        }
    }

    /// Checks if it an ordinary object.
    #[inline]
    pub fn is_ordinary(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::Ordinary,
                ..
            }
        )
    }

    /// Gets the prototype instance of this object.
    #[inline]
    pub fn prototype(&self) -> &JsPrototype {
        &self.prototype
    }

    /// Sets the prototype instance of the object.
    ///
    /// [More information][spec]
    ///
    /// [spec]: https://tc39.es/ecma262/#sec-invariants-of-the-essential-internal-methods
    #[inline]
    #[track_caller]
    pub fn set_prototype<O: Into<JsPrototype>>(&mut self, prototype: O) -> bool {
        let prototype = prototype.into();
        if self.extensible {
            self.prototype = prototype;
            true
        } else {
            // If target is non-extensible, [[SetPrototypeOf]] must return false
            // unless V is the SameValue as the target's observed [[GetPrototypeOf]] value.
            self.prototype == prototype
        }
    }

    /// Returns `true` if it holds an Rust type that implements `NativeObject`.
    #[inline]
    pub fn is_native_object(&self) -> bool {
        matches!(
            self.data,
            ObjectData {
                kind: ObjectKind::NativeObject(_),
                ..
            }
        )
    }

    #[inline]
    pub fn as_native_object(&self) -> Option<&dyn NativeObject> {
        match self.data {
            ObjectData {
                kind: ObjectKind::NativeObject(ref object),
                ..
            } => Some(object.as_ref()),
            _ => None,
        }
    }

    /// Reeturn `true` if it is a native object and the native type is `T`.
    #[inline]
    pub fn is<T>(&self) -> bool
    where
        T: NativeObject,
    {
        match self.data {
            ObjectData {
                kind: ObjectKind::NativeObject(ref object),
                ..
            } => object.deref().as_any().is::<T>(),
            _ => false,
        }
    }

    /// Downcast a reference to the object,
    /// if the object is type native object type `T`.
    #[inline]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: NativeObject,
    {
        match self.data {
            ObjectData {
                kind: ObjectKind::NativeObject(ref object),
                ..
            } => object.deref().as_any().downcast_ref::<T>(),
            _ => None,
        }
    }

    /// Downcast a mutable reference to the object,
    /// if the object is type native object type `T`.
    #[inline]
    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: NativeObject,
    {
        match self.data {
            ObjectData {
                kind: ObjectKind::NativeObject(ref mut object),
                ..
            } => object.deref_mut().as_mut_any().downcast_mut::<T>(),
            _ => None,
        }
    }

    #[inline]
    pub fn properties(&self) -> &PropertyMap {
        &self.properties
    }

    /// Helper function for property insertion.
    #[inline]
    pub(crate) fn insert<K, P>(&mut self, key: K, property: P) -> Option<PropertyDescriptor>
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        self.properties.insert(key.into(), property.into())
    }

    /// Helper function for property removal.
    #[inline]
    pub(crate) fn remove(&mut self, key: &PropertyKey) -> Option<PropertyDescriptor> {
        self.properties.remove(key)
    }

    /// Inserts a field in the object `properties` without checking if it's writable.
    ///
    /// If a field was already in the object with the same name that a `Some` is returned
    /// with that field, otherwise None is retuned.
    #[inline]
    pub fn insert_property<K, P>(&mut self, key: K, property: P) -> Option<PropertyDescriptor>
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        self.insert(key, property)
    }
}

/// The functions binding.
///
/// Specifies what is the name of the function object (`name` property),
/// and the binding name of the function object which can be different
/// from the function name.
///
/// The only way to construct this is with the `From` trait.
///
/// There are two implementations:
///  - From a single type `T` which implements `Into<FunctionBinding>` which sets the binding
/// name and the function name to the same value
///  - From a tuple `(B: Into<PropertyKey>, N: AsRef<str>)` the `B` is the binding name
/// and the `N` is the function name.
#[derive(Debug, Clone)]
pub struct FunctionBinding {
    binding: PropertyKey,
    name: JsString,
}

impl From<&str> for FunctionBinding {
    #[inline]
    fn from(name: &str) -> Self {
        let name: JsString = name.into();

        Self {
            binding: name.clone().into(),
            name,
        }
    }
}

impl From<String> for FunctionBinding {
    #[inline]
    fn from(name: String) -> Self {
        let name: JsString = name.into();

        Self {
            binding: name.clone().into(),
            name,
        }
    }
}

impl From<JsString> for FunctionBinding {
    #[inline]
    fn from(name: JsString) -> Self {
        Self {
            binding: name.clone().into(),
            name,
        }
    }
}

impl<B, N> From<(B, N)> for FunctionBinding
where
    B: Into<PropertyKey>,
    N: AsRef<str>,
{
    #[inline]
    fn from((binding, name): (B, N)) -> Self {
        Self {
            binding: binding.into(),
            name: name.as_ref().into(),
        }
    }
}

/// Builder for creating native function objects
#[derive(Debug)]
pub struct FunctionBuilder<'context> {
    context: &'context mut Context,
    function: Option<Function>,
    name: JsString,
    length: usize,
}

impl<'context> FunctionBuilder<'context> {
    /// Create a new `FunctionBuilder` for creating a native function.
    #[inline]
    pub fn native(context: &'context mut Context, function: NativeFunctionSignature) -> Self {
        Self {
            context,
            function: Some(Function::Native {
                function,
                constructor: false,
            }),
            name: JsString::default(),
            length: 0,
        }
    }

    /// Create a new `FunctionBuilder` for creating a closure function.
    #[inline]
    pub fn closure<F>(context: &'context mut Context, function: F) -> Self
    where
        F: Fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue> + Copy + 'static,
    {
        Self {
            context,
            function: Some(Function::Closure {
                function: Box::new(move |this, args, _, context| function(this, args, context)),
                constructor: false,
                captures: Captures::new(()),
            }),
            name: JsString::default(),
            length: 0,
        }
    }

    /// Create a new closure function with additional captures.
    ///
    /// # Note
    ///
    /// You can only move variables that implement `Debug + Any + Trace + Clone`.
    /// In other words, only `NativeObject + Clone` objects are movable.
    #[inline]
    pub fn closure_with_captures<F, C>(
        context: &'context mut Context,
        function: F,
        captures: C,
    ) -> Self
    where
        F: Fn(&JsValue, &[JsValue], &mut C, &mut Context) -> JsResult<JsValue> + Copy + 'static,
        C: NativeObject,
    {
        Self {
            context,
            function: Some(Function::Closure {
                function: Box::new(move |this, args, captures: Captures, context| {
                    let mut captures = captures.as_mut_any();
                    let captures = captures.downcast_mut::<C>().ok_or_else(|| {
                        context.construct_type_error("cannot downcast `Captures` to given type")
                    })?;
                    function(this, args, captures, context)
                }),
                constructor: false,
                captures: Captures::new(captures),
            }),
            name: JsString::default(),
            length: 0,
        }
    }

    /// Specify the name property of object function object.
    ///
    /// The default is `""` (empty string).
    #[inline]
    pub fn name<N>(&mut self, name: N) -> &mut Self
    where
        N: AsRef<str>,
    {
        self.name = name.as_ref().into();
        self
    }

    /// Specify the length property of object function object.
    ///
    /// How many arguments this function takes.
    ///
    /// The default is `0`.
    #[inline]
    pub fn length(&mut self, length: usize) -> &mut Self {
        self.length = length;
        self
    }

    /// Specify whether the object function object can be called with `new` keyword.
    ///
    /// The default is `false`.
    #[inline]
    pub fn constructor(&mut self, yes: bool) -> &mut Self {
        match self.function.as_mut() {
            Some(Function::Native { constructor, .. }) => *constructor = yes,
            Some(Function::Closure { constructor, .. }) => *constructor = yes,
            _ => unreachable!(),
        }
        self
    }

    /// Build the function object.
    #[inline]
    pub fn build(&mut self) -> JsObject {
        let function = JsObject::from_proto_and_data(
            self.context
                .standard_objects()
                .function_object()
                .prototype(),
            ObjectData::function(self.function.take().unwrap()),
        );
        let property = PropertyDescriptor::builder()
            .writable(false)
            .enumerable(false)
            .configurable(true);
        function.insert_property("name", property.clone().value(self.name.clone()));
        function.insert_property("length", property.value(self.length));

        function
    }

    /// Initializes the `Function.prototype` function object.
    pub(crate) fn build_function_prototype(&mut self, object: &JsObject) {
        let mut object = object.borrow_mut();
        object.data = ObjectData::function(self.function.take().unwrap());
        object.set_prototype(self.context.standard_objects().object_object().prototype());

        let property = PropertyDescriptor::builder()
            .writable(false)
            .enumerable(false)
            .configurable(true);
        object.insert_property("name", property.clone().value(self.name.clone()));
        object.insert_property("length", property.value(self.length));
    }
}

/// Builder for creating objects with properties.
///
/// # Examples
///
/// ```
/// # use boa::{Context, JsValue, object::ObjectInitializer, property::Attribute};
/// let mut context = Context::new();
/// let object = ObjectInitializer::new(&mut context)
///     .property(
///         "hello",
///         "world",
///         Attribute::all()
///     )
///     .property(
///         1,
///         1,
///         Attribute::all()
///     )
///     .function(|_, _, _| Ok(JsValue::undefined()), "func", 0)
///     .build();
/// ```
///
/// The equivalent in JavaScript would be:
/// ```text
/// let object = {
///     hello: "world",
///     "1": 1,
///     func: function() {}
/// }
/// ```
#[derive(Debug)]
pub struct ObjectInitializer<'context> {
    context: &'context mut Context,
    object: JsObject,
}

impl<'context> ObjectInitializer<'context> {
    /// Create a new `ObjectBuilder`.
    #[inline]
    pub fn new(context: &'context mut Context) -> Self {
        let object = context.construct_object();
        Self { context, object }
    }

    /// Add a function to the object.
    #[inline]
    pub fn function<B>(
        &mut self,
        function: NativeFunctionSignature,
        binding: B,
        length: usize,
    ) -> &mut Self
    where
        B: Into<FunctionBinding>,
    {
        let binding = binding.into();
        let function = FunctionBuilder::native(self.context, function)
            .name(binding.name)
            .length(length)
            .constructor(false)
            .build();

        self.object.borrow_mut().insert_property(
            binding.binding,
            PropertyDescriptor::builder()
                .value(function)
                .writable(true)
                .enumerable(false)
                .configurable(true),
        );
        self
    }

    /// Add a property to the object.
    #[inline]
    pub fn property<K, V>(&mut self, key: K, value: V, attribute: Attribute) -> &mut Self
    where
        K: Into<PropertyKey>,
        V: Into<JsValue>,
    {
        let property = PropertyDescriptor::builder()
            .value(value)
            .writable(attribute.writable())
            .enumerable(attribute.enumerable())
            .configurable(attribute.configurable());
        self.object.borrow_mut().insert(key, property);
        self
    }

    /// Build the object.
    #[inline]
    pub fn build(&mut self) -> JsObject {
        self.object.clone()
    }
}

/// Builder for creating constructors objects, like `Array`.
pub struct ConstructorBuilder<'context> {
    context: &'context mut Context,
    constructor_function: NativeFunctionSignature,
    constructor_object: JsObject,
    prototype: JsObject,
    name: JsString,
    length: usize,
    callable: bool,
    constructor: bool,
    inherit: Option<JsPrototype>,
    custom_prototype: Option<JsPrototype>,
}

impl Debug for ConstructorBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConstructorBuilder")
            .field("name", &self.name)
            .field("length", &self.length)
            .field("constructor", &self.constructor_object)
            .field("prototype", &self.prototype)
            .field("inherit", &self.inherit)
            .field("callable", &self.callable)
            .field("constructor", &self.constructor)
            .finish()
    }
}

impl<'context> ConstructorBuilder<'context> {
    /// Create a new `ConstructorBuilder`.
    #[inline]
    pub fn new(context: &'context mut Context, constructor: NativeFunctionSignature) -> Self {
        Self {
            context,
            constructor_function: constructor,
            constructor_object: JsObject::empty(),
            prototype: JsObject::empty(),
            length: 0,
            name: JsString::default(),
            callable: true,
            constructor: true,
            inherit: None,
            custom_prototype: None,
        }
    }

    #[inline]
    pub(crate) fn with_standard_object(
        context: &'context mut Context,
        constructor: NativeFunctionSignature,
        object: StandardConstructor,
    ) -> Self {
        Self {
            context,
            constructor_function: constructor,
            constructor_object: object.constructor,
            prototype: object.prototype,
            length: 0,
            name: JsString::default(),
            callable: true,
            constructor: true,
            inherit: None,
            custom_prototype: None,
        }
    }

    /// Add new method to the constructors prototype.
    #[inline]
    pub fn method<B>(
        &mut self,
        function: NativeFunctionSignature,
        binding: B,
        length: usize,
    ) -> &mut Self
    where
        B: Into<FunctionBinding>,
    {
        let binding = binding.into();
        let function = FunctionBuilder::native(self.context, function)
            .name(binding.name)
            .length(length)
            .constructor(false)
            .build();

        self.prototype.borrow_mut().insert_property(
            binding.binding,
            PropertyDescriptor::builder()
                .value(function)
                .writable(true)
                .enumerable(false)
                .configurable(true),
        );
        self
    }

    /// Add new static method to the constructors object itself.
    #[inline]
    pub fn static_method<B>(
        &mut self,
        function: NativeFunctionSignature,
        binding: B,
        length: usize,
    ) -> &mut Self
    where
        B: Into<FunctionBinding>,
    {
        let binding = binding.into();
        let function = FunctionBuilder::native(self.context, function)
            .name(binding.name)
            .length(length)
            .constructor(false)
            .build();

        self.constructor_object.borrow_mut().insert_property(
            binding.binding,
            PropertyDescriptor::builder()
                .value(function)
                .writable(true)
                .enumerable(false)
                .configurable(true),
        );
        self
    }

    /// Add new data property to the constructor's prototype.
    #[inline]
    pub fn property<K, V>(&mut self, key: K, value: V, attribute: Attribute) -> &mut Self
    where
        K: Into<PropertyKey>,
        V: Into<JsValue>,
    {
        let property = PropertyDescriptor::builder()
            .value(value)
            .writable(attribute.writable())
            .enumerable(attribute.enumerable())
            .configurable(attribute.configurable());
        self.prototype.borrow_mut().insert(key, property);
        self
    }

    /// Add new static data property to the constructor object itself.
    #[inline]
    pub fn static_property<K, V>(&mut self, key: K, value: V, attribute: Attribute) -> &mut Self
    where
        K: Into<PropertyKey>,
        V: Into<JsValue>,
    {
        let property = PropertyDescriptor::builder()
            .value(value)
            .writable(attribute.writable())
            .enumerable(attribute.enumerable())
            .configurable(attribute.configurable());
        self.constructor_object.borrow_mut().insert(key, property);
        self
    }

    /// Add new accessor property to the constructor's prototype.
    #[inline]
    pub fn accessor<K>(
        &mut self,
        key: K,
        get: Option<JsObject>,
        set: Option<JsObject>,
        attribute: Attribute,
    ) -> &mut Self
    where
        K: Into<PropertyKey>,
    {
        let property = PropertyDescriptor::builder()
            .maybe_get(get)
            .maybe_set(set)
            .enumerable(attribute.enumerable())
            .configurable(attribute.configurable());
        self.prototype.borrow_mut().insert(key, property);
        self
    }

    /// Add new static accessor property to the constructor object itself.
    #[inline]
    pub fn static_accessor<K>(
        &mut self,
        key: K,
        get: Option<JsObject>,
        set: Option<JsObject>,
        attribute: Attribute,
    ) -> &mut Self
    where
        K: Into<PropertyKey>,
    {
        let property = PropertyDescriptor::builder()
            .maybe_get(get)
            .maybe_set(set)
            .enumerable(attribute.enumerable())
            .configurable(attribute.configurable());
        self.constructor_object.borrow_mut().insert(key, property);
        self
    }

    /// Add new property to the constructor's prototype.
    #[inline]
    pub fn property_descriptor<K, P>(&mut self, key: K, property: P) -> &mut Self
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        let property = property.into();
        self.prototype.borrow_mut().insert(key, property);
        self
    }

    /// Add new static property to the constructor object itself.
    #[inline]
    pub fn static_property_descriptor<K, P>(&mut self, key: K, property: P) -> &mut Self
    where
        K: Into<PropertyKey>,
        P: Into<PropertyDescriptor>,
    {
        let property = property.into();
        self.constructor_object.borrow_mut().insert(key, property);
        self
    }

    /// Specify how many arguments the constructor function takes.
    ///
    /// Default is `0`.
    #[inline]
    pub fn length(&mut self, length: usize) -> &mut Self {
        self.length = length;
        self
    }

    /// Specify the name of the constructor function.
    ///
    /// Default is `"[object]"`
    #[inline]
    pub fn name<N>(&mut self, name: N) -> &mut Self
    where
        N: AsRef<str>,
    {
        self.name = name.as_ref().into();
        self
    }

    /// Specify whether the constructor function can be called.
    ///
    /// Default is `true`
    #[inline]
    pub fn callable(&mut self, callable: bool) -> &mut Self {
        self.callable = callable;
        self
    }

    /// Specify whether the constructor function can be called with `new` keyword.
    ///
    /// Default is `true`
    #[inline]
    pub fn constructor(&mut self, constructor: bool) -> &mut Self {
        self.constructor = constructor;
        self
    }

    /// Specify the prototype this constructor object inherits from.
    ///
    /// Default is `Object.prototype`
    #[inline]
    pub fn inherit<O: Into<JsPrototype>>(&mut self, prototype: O) -> &mut Self {
        self.inherit = Some(prototype.into());
        self
    }

    /// Specify the __proto__ for this constructor.
    ///
    /// Default is `Function.prototype`
    #[inline]
    pub fn custom_prototype<O: Into<JsPrototype>>(&mut self, prototype: O) -> &mut Self {
        self.custom_prototype = Some(prototype.into());
        self
    }

    /// Return the current context.
    #[inline]
    pub fn context(&mut self) -> &'_ mut Context {
        self.context
    }

    /// Build the constructor function object.
    pub fn build(&mut self) -> JsObject {
        // Create the native function
        let function = Function::Native {
            function: self.constructor_function,
            constructor: self.constructor,
        };

        let length = PropertyDescriptor::builder()
            .value(self.length)
            .writable(false)
            .enumerable(false)
            .configurable(true);
        let name = PropertyDescriptor::builder()
            .value(self.name.clone())
            .writable(false)
            .enumerable(false)
            .configurable(true);

        {
            let mut constructor = self.constructor_object.borrow_mut();
            constructor.data = ObjectData::function(function);
            constructor.insert("length", length);
            constructor.insert("name", name);

            if let Some(proto) = self.custom_prototype.take() {
                constructor.set_prototype(proto);
            } else {
                constructor.set_prototype(
                    self.context
                        .standard_objects()
                        .function_object()
                        .prototype(),
                );
            }
            constructor.insert_property(
                PROTOTYPE,
                PropertyDescriptor::builder()
                    .value(self.prototype.clone())
                    .writable(false)
                    .enumerable(false)
                    .configurable(false),
            );
        }

        {
            let mut prototype = self.prototype.borrow_mut();
            prototype.insert_property(
                "constructor",
                PropertyDescriptor::builder()
                    .value(self.constructor_object.clone())
                    .writable(true)
                    .enumerable(false)
                    .configurable(true),
            );

            if let Some(proto) = self.inherit.take() {
                prototype.set_prototype(proto);
            } else {
                prototype
                    .set_prototype(self.context.standard_objects().object_object().prototype());
            }
        }

        self.constructor_object.clone()
    }
}

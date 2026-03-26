//!
//! Utility macros
//!

/// Create an enum that can be encoded/decoded, and implement utility traits.
///
/// This works for enums that are represented as int types (u8, u16, ...).
/// The value will be stored in a newtype struct, and an enum with the name will be generated.
///
/// This also generates the constants and implements a custom Display and Debug implementations
/// to display the known names, or the value as hexadecimal.
///
/// # Example
///
/// ```
/// codec_enum! {
///
///     // The type to store the value
///     pub struct MyEnum(pub(crate) u16);
///
///     // The enum with the constants
///     pub enum MyEnumName {
///         SOME_VALUE = 12,
///         ANOTHER_VALUE = 15,
///     }
/// }
///
/// fn foo() {
///
///     // You can then use a few utility traits :
///
///     // Constant values for the struct
///     let a = MyEnum::SOME_VALUE;
///
///     // Convert the value to its name
///     assert_eq!(MyEnumName::SOME_VALUE, MyEnumName::try_from(a).unwrap());
///
///     // Convert a name to its value
///     assert_eq!(MyEnum::SOME_VALUE, MyEnumName::SOME_VALUE.into());
///
///     // Decode and encode values
///     let mut buffer = Vec::new();
///     
///     a.encode(&mut buffer).unwrap();
///
///     let mut reader = Reader::new(&buffer);
///     let a_decoded = MyEnum::decode(&mut reader).unwrap();
///
///     assert_eq!(a, a_decoded);
///
///     // Display values
///     assert_eq!(format!("{}", MyEnum::SOME_VALUE), "SOME_VALUE".into());
///     assert_eq!(format!("{}", MyEnum(12)), "SOME_VALUE".into());
///     assert_eq!(format!("{}", MyEnum(42)), "MyEnum(0x002A)".into());
/// }
/// ```
macro_rules! codec_enum {
    {
        $(#[doc = $comment:literal])?
        $struct_vis:vis struct $struct_name:ident($inner_vis:vis $utype:ty);

        $(#[$enum_meta:meta])*
        $enum_vis:vis enum $enum_name:ident {
            $(
                $(#[$var_meta:meta])*
                $var_name:ident = $var_value:literal
            ),*

            // Accept a terminating ',' even if there is only one variant
            $(,)?
        }
    } => {

        // struct definition
        $(#[doc = $comment])?
        #[derive(::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq)]
        $struct_vis struct $struct_name($inner_vis $utype);

        // enum definition
        $(#[doc = $comment])?
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq)]
        #[repr($utype)]
        $(#[$enum_meta])*
        $enum_vis enum $enum_name {
            $(
                $(#[$var_meta])*
                $var_name = $var_value
            ),*
        }

        // Constants
        impl $struct_name {
            $(
                #[allow(unused, non_upper_case_globals)]
                $struct_vis const $var_name: Self = Self($var_value);
            )*
        }

        // Conversion between struct and enum

        impl ::core::convert::From<$enum_name> for $struct_name {
            #[inline]
            fn from(value: $enum_name) -> Self {
                $struct_name(value as $utype)
            }
        }

        impl ::core::convert::TryFrom<$struct_name> for $enum_name {
            type Error = ();

            fn try_from(value: $struct_name) -> ::core::result::Result<Self, Self::Error> {
                use self::$enum_name::*;
                Ok(match value.0 {
                    $($var_value => $var_name,)*
                    _ => return Err(()),
                })
            }
        }

        // Encoding / decoding
        impl crate::codec::Codec for $struct_name {

            const LENGTH_HINT: Option<usize> = Some(::std::mem::size_of::<$utype>());

            #[inline]
            fn encode(&self, dest: &mut ::std::vec::Vec<u8>) {
                self.0.encode(dest)
            }

            #[inline]
            fn decode(buf: &mut crate::reader::Reader) -> Result<Self, crate::error::InvalidMessage> {
                Ok($struct_name(<$utype>::decode(buf)?))
            }

            #[inline]
            fn encoded_length_hint(&self) -> ::core::option::Option<usize> {
                Some(::std::mem::size_of::<$utype>())
            }
        }

        // Display
        impl ::core::fmt::Debug for $struct_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self.0 {

                    // Known values
                    $(
                        $var_value => ::core::write!(f, stringify!($var_name)),
                    )*

                    // Unknown values
                    v => write!(f,
                        "{}(0x{:0width$x})",
                        stringify!($struct_name),
                        v,
                        width = ::std::mem::size_of::<$utype>() * 2
                    ),
                }
            }
        }
    };
}

pub(crate) use codec_enum;

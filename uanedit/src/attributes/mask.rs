/// Defines a bitmask attribute as a transparent newtype.
///
/// The raw integer is kept rather than a set of known flags, so a mask carrying bits from a
/// revision this crate predates is written back out intact.
macro_rules! bitmask {
    (
        $(#[$meta:meta])*
        $name:ident: $repr:ty {
            $($flag:ident = $bit:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
            serde::Serialize, serde::Deserialize,
        )]
        pub struct $name(pub $repr);

        impl $name {
            pub const NONE: Self = Self(0);
            $(pub const $flag: Self = Self(1 << $bit);)*

            /// Every flag this crate knows, in the order the spec defines them.
            pub const FLAGS: &'static [(&'static str, Self)] = &[$((stringify!($flag), Self::$flag)),*];

            pub fn bits(self) -> $repr {
                self.0
            }

            pub fn is_empty(self) -> bool {
                self.0 == 0
            }

            pub fn contains(self, flags: Self) -> bool {
                self.0 & flags.0 == flags.0
            }

            pub fn with(self, flags: Self) -> Self {
                Self(self.0 | flags.0)
            }

            pub fn without(self, flags: Self) -> Self {
                Self(self.0 & !flags.0)
            }

            pub fn set(&mut self, flags: Self, enabled: bool) {
                *self = if enabled { self.with(flags) } else { self.without(flags) };
            }

            /// The names of the set flags this crate knows.
            pub fn names(self) -> impl Iterator<Item = &'static str> {
                Self::FLAGS
                    .iter()
                    .filter(move |(_, flag)| self.contains(*flag))
                    .map(|(name, _)| *name)
            }

            /// The set bits this crate has no name for, which a newer revision may define.
            pub fn unknown_bits(self) -> $repr {
                let known = Self::FLAGS.iter().fold(0, |known, (_, flag)| known | flag.0);
                self.0 & !known
            }
        }

        impl From<$repr> for $name {
            fn from(bits: $repr) -> Self {
                Self(bits)
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, other: Self) -> Self {
                self.with(other)
            }
        }

        impl core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, other: Self) {
                *self = self.with(other);
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut written = false;
                for name in self.names() {
                    if written {
                        f.write_str(" | ")?;
                    }
                    f.write_str(name)?;
                    written = true;
                }
                let unknown = self.unknown_bits();
                if unknown != 0 {
                    if written {
                        f.write_str(" | ")?;
                    }
                    return write!(f, "{unknown:#x}");
                }
                if !written {
                    f.write_str("None")?;
                }
                Ok(())
            }
        }
    };
}

pub(crate) use bitmask;
